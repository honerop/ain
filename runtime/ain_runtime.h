/* AIN bootstrap C runtime. Stage 1/2/3 use this same tiny trusted ABI.
 * Memory is reclaimed at process exit; suitable for compilation tasks, not long-lived servers.
 */
#ifndef AIN_RUNTIME_H
#define AIN_RUNTIME_H
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>
#include <ctype.h>
#include <limits.h>

typedef struct { int tag; int64_t i; const char *s; } V;
enum { T_UNIT, T_INT, T_BOOL, T_STR };
static void v_fail(const char *message) { fprintf(stderr, "ain runtime: %s\n", message); exit(1); }
static V v_unit(void) { return (V){T_UNIT, 0, NULL}; }
static V v_int(int64_t n) { return (V){T_INT, n, NULL}; }
static V v_bool(int n) { return (V){T_BOOL, !!n, NULL}; }
static V v_str(const char *s) { return (V){T_STR, 0, s}; }
static int64_t v_n(V x) { if (x.tag != T_INT) v_fail("expected integer"); return x.i; }
static const char *v_s(V x) { if (x.tag != T_STR) v_fail("expected string"); return x.s; }
static int v_truth(V x) { if (x.tag != T_BOOL) v_fail("expected boolean"); return x.i != 0; }
static char *v_alloc(size_t n) { char *p = malloc(n); if (!p) v_fail("out of memory"); return p; }
static char *v_dup_n(const char *s, size_t n) { char *p=v_alloc(n+1); memcpy(p,s,n); p[n]='\0'; return p; }
static V v_concat(V a, V b) {
 const char *x=v_s(a), *y=v_s(b); size_t n=strlen(x), m=strlen(y);
 if (n>SIZE_MAX-m-1) v_fail("string too large");
 char *p=v_alloc(n+m+1); memcpy(p,x,n); memcpy(p+n,y,m+1); return v_str(p);
}
static V v_before(V source, V sep) {
 const char *s=v_s(source), *q=v_s(sep);
 if (!*q) return v_str("");
 const char *p=strstr(s,q); return v_str(p?v_dup_n(s,(size_t)(p-s)):s);
}
static V v_after(V source, V sep) {
 const char *s=v_s(source), *q=v_s(sep);
 if (!*q) return source;
 const char *p=strstr(s,q); return v_str(p?p+strlen(q):"");
}
static const char *v_white(const char *s) { while (isspace((unsigned char)*s)) s++; return s; }
static const char *v_token_end(const char *s) {
 s=v_white(s); if (!*s || *s=='#') return s;
 if (*s=='"') {
  s++; int esc=0;
  while (*s) { if (esc) { esc=0; s++; continue; }
   if (*s=='\\') { esc=1; s++; continue; }
   if (*s=='"') return s+1;
   s++;
  }
  v_fail("unterminated quoted token");
 }
 while (*s && !isspace((unsigned char)*s) && *s!='#' && *s!='"') s++;
 return s;
}
static V v_word(V input) { const char *s=v_white(v_s(input)), *end=v_token_end(s); return v_str(v_dup_n(s,(size_t)(end-s))); }
static V v_skip(V input) { const char *s=v_white(v_s(input)), *end=v_token_end(s); return v_str(v_white(end)); }
static V v_num(V input) {
 const char *s=v_s(input); char *end=NULL; if(!*s) v_fail("empty integer");
 long long n=strtoll(s,&end,10); if(!end||*end) v_fail("invalid integer"); return v_int((int64_t)n);
}
static V v_isnum(V input) {
 const char *s=v_s(input); char *end=NULL;
 if(!*s) return v_bool(0);
 (void)strtoll(s,&end,10); return v_bool(end && !*end && end!=s);
}
static V v_input(V ignored) {
 (void)ignored; size_t capacity=4096,size=0; char *buf=v_alloc(capacity);
 for (;;) { if (size+2048+1>capacity) { capacity*=2; char *newbuf=realloc(buf,capacity); if (!newbuf) v_fail("out of memory"); buf=newbuf; }
 size_t n=fread(buf+size,1,2048,stdin); size+=n;
 if (n<2048) { if (ferror(stdin)) v_fail("stdin read failed"); break; }
 }
 buf[size]=0; return v_str(buf);
}
static V v_str_value(V x) {
 char buf[64];
 if (x.tag==T_STR) return x;
 if (x.tag==T_UNIT) return v_str("()");
 if (x.tag==T_BOOL) return v_str(x.i?"true":"false");
 snprintf(buf,sizeof(buf),"%lld",(long long)x.i); return v_str(v_dup_n(buf,strlen(buf)));
}
static V v_print(V x) { V s=v_str_value(x); puts(v_s(s)); return v_unit(); }
static V v_add(V a,V b) { int64_t x; if(__builtin_add_overflow(v_n(a),v_n(b),&x)) v_fail("integer overflow"); return v_int(x); }
static V v_sub(V a,V b) { int64_t x; if(__builtin_sub_overflow(v_n(a),v_n(b),&x)) v_fail("integer overflow"); return v_int(x); }
static V v_mul(V a,V b) { int64_t x; if(__builtin_mul_overflow(v_n(a),v_n(b),&x)) v_fail("integer overflow"); return v_int(x); }
static V v_div(V a,V b) { int64_t x=v_n(a),y=v_n(b); if(!y||(x==INT64_MIN&&y==-1)) v_fail("invalid division"); return v_int(x/y); }
static V v_mod(V a,V b) { int64_t x=v_n(a),y=v_n(b); if(!y||(x==INT64_MIN&&y==-1)) v_fail("invalid remainder"); return v_int(x%y); }
static V v_neg(V a) { int64_t x=v_n(a); if(x==INT64_MIN)v_fail("integer overflow"); return v_int(-x); }
static V v_abs(V a) { int64_t x=v_n(a); if(x==INT64_MIN)v_fail("integer overflow"); return v_int(x<0?-x:x); }
static V v_min(V a,V b) { int64_t x=v_n(a),y=v_n(b);return v_int(x<y?x:y); }
static V v_max(V a,V b) { int64_t x=v_n(a),y=v_n(b);return v_int(x>y?x:y); }
static V v_pow(V a,V b) { int64_t n=v_n(b),out=1,x=v_n(a); if(n<0||n>UINT32_MAX)v_fail("invalid exponent"); while(n){if(n&1){if(__builtin_mul_overflow(out,x,&out))v_fail("integer overflow");}n>>=1;if(n&&__builtin_mul_overflow(x,x,&x))v_fail("integer overflow");}return v_int(out); }
static V v_clamp(V x,V low,V high) {int64_t a=v_n(x),b=v_n(low),c=v_n(high);if(b>c)v_fail("invalid clamp bounds");return v_int(a<b?b:a>c?c:a);}
static V v_eq(V a,V b) { if(a.tag!=b.tag)return v_bool(0); if(a.tag==T_STR)return v_bool(!strcmp(a.s,b.s)); return v_bool(a.i==b.i); }
static V v_ne(V a,V b) {return v_bool(!v_truth(v_eq(a,b)));}
static V v_lt(V a,V b) {return v_bool(v_n(a)<v_n(b));}
static V v_le(V a,V b) {return v_bool(v_n(a)<=v_n(b));}
static V v_gt(V a,V b) {return v_bool(v_n(a)>v_n(b));}
static V v_ge(V a,V b) {return v_bool(v_n(a)>=v_n(b));}
static V v_not(V a) {return v_bool(!v_truth(a));}
#define v_if(c,a,b) (v_truth((c)) ? (a) : (b))
#define v_and(a,b) (v_truth((a)) ? v_bool(v_truth((b))) : v_bool(0))
#define v_or(a,b) (v_truth((a)) ? v_bool(1) : v_bool(v_truth((b))))
static V v_str_(V a) {return v_str_value(a);}
#define v_str_cast(a) v_str_value(a)
static V v_len(V a) {const unsigned char *s=(const unsigned char*)v_s(a);int64_t count=0;while(*s){if((*s&0xc0)!=0x80)count++;s++;}return v_int(count);}
static V v_upper(V a) {const char *s=v_s(a);char *p=v_dup_n(s,strlen(s));for(char *q=p;*q;q++)*q=(char)toupper((unsigned char)*q);return v_str(p);}
static V v_lower(V a) {const char *s=v_s(a);char *p=v_dup_n(s,strlen(s));for(char *q=p;*q;q++)*q=(char)tolower((unsigned char)*q);return v_str(p);}
static V v_trim(V a) {const char *s=v_white(v_s(a));size_t n=strlen(s);while(n&&isspace((unsigned char)s[n-1]))n--;return v_str(v_dup_n(s,n));}
static V v_contains(V a,V b){return v_bool(strstr(v_s(a),v_s(b))!=NULL);}
static V v_starts(V a,V b){const char *s=v_s(a),*p=v_s(b);return v_bool(strncmp(s,p,strlen(p))==0);}
static V v_ends(V a,V b){const char *s=v_s(a),*p=v_s(b);size_t n=strlen(s),m=strlen(p);return v_bool(n>=m&&!strcmp(s+n-m,p));}
static V v_replace(V a,V b,V c){const char *s=v_s(a),*old=v_s(b),*newtext=v_s(c);size_t n=strlen(old);if(!n)return a;V out=v_str("");while(1){const char *hit=strstr(s,old);if(!hit)return v_concat(out,v_str(s));out=v_concat(out,v_str(v_dup_n(s,(size_t)(hit-s))));out=v_concat(out,v_str(newtext));s=hit+n;}}
/* The desktop GUI host is currently supported by the Rust backend only.
 * Fail explicitly instead of pretending it was launched in the C bootstrap. */
static V v_desktop(V title,V system,V page) {
 (void)title; (void)system; (void)page;
 v_fail("desktop is only available through AIN's Rust backend");
 return v_unit();
}
#endif
