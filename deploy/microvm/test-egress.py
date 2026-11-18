#!/usr/bin/env python3
"""Exercise policy compilation without changing a host firewall."""
import json
import runpy
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parent
rules = runpy.run_path(str(ROOT / "install-egress.py"))["rules"]

class EgressPolicy(unittest.TestCase):
    def setUp(self):
        self.policy = json.loads((ROOT / "egress.example.json").read_text())

    def test_only_reply_traffic_can_enter_guest_and_default_forward_is_closed(self):
        self.policy["allow"].append({"destination": "192.0.2.0/24", "protocol": "tcp", "port": 443})
        result = rules(self.policy)
        self.assertIn("hook forward priority filter; policy drop", result)
        self.assertIn('iifname { "calvm0", "calvm1" } drop', result)
        self.assertIn('oifname { "calvm0", "calvm1" } drop', result)
        self.assertIn(
            'oifname "calvm0" ip daddr 10.213.0.2 ct direction reply ct state established,related accept',
            result,
        )
        self.assertIn("ip daddr 192.0.2.0/24 tcp dport 443 accept", result)
        self.assertLess(result.index("169.254.0.0/16"), result.index("192.0.2.0/24"))

    def test_rejects_two_taps_on_the_same_subnet(self):
        self.policy["slots"][1].update(address="10.213.0.1", gateway="10.213.0.2")
        with self.assertRaises(ValueError):
            rules(self.policy)

if __name__ == "__main__":
    unittest.main()
