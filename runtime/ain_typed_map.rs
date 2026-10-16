// Type-inferred maps with deterministic key ordering and persistent-style updates.
fn ain_map_one<T>(key: String, value: T) -> std::collections::BTreeMap<String,T> {
    std::collections::BTreeMap::from([(key, value)])
}
fn ain_map_set<T>(mut map: std::collections::BTreeMap<String,T>, key: String, value: T)
    -> std::collections::BTreeMap<String,T> {
    map.insert(key, value); map
}
fn ain_map_at<T: Clone>(map: std::collections::BTreeMap<String,T>, key: String, fallback: T) -> T {
    map.get(&key).cloned().unwrap_or(fallback)
}
fn ain_map_has<T>(map: std::collections::BTreeMap<String,T>, key: String) -> bool {
    map.contains_key(&key)
}
fn ain_map_remove<T>(mut map: std::collections::BTreeMap<String,T>, key: String)
    -> std::collections::BTreeMap<String,T> {
    map.remove(&key); map
}
fn ain_map_len<T>(map: std::collections::BTreeMap<String,T>) -> i64 { map.len() as i64 }
fn ain_map_keys<T>(map: std::collections::BTreeMap<String,T>) -> Vec<String> {
    map.into_keys().collect()
}
fn ain_map_values<T>(map: std::collections::BTreeMap<String,T>) -> Vec<T> {
    map.into_values().collect()
}
