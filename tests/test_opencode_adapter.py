import unittest
from scripts.opencode_adapter import make_prompt, extract_source

class OpenCodeAdapterTests(unittest.TestCase):
    def test_extracts_only_text_events_and_strips_code_fences(self):
        stream = ('{"type":"step_start","part":{}}\n'
                  '{"type":"text","part":{"text":"```ain\nprint 42\n```"}}\n')
        self.assertEqual(extract_source(stream), "print 42\n")

    def test_includes_compiler_diagnostics_on_retry(self):
        prompt = make_prompt({"prompt": "Print 42", "previous_source": "print foo",
                              "diagnostics": [{"code": "E_UNKNOWN_VARIABLE"}]})
        self.assertIn("print foo", prompt)
        self.assertIn("E_UNKNOWN_VARIABLE", prompt)

    def test_rejects_provider_errors(self):
        with self.assertRaises(ValueError):
            extract_source('{"type":"error","message":"quota"}')

if __name__ == "__main__":
    unittest.main()
