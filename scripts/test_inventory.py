import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("inventory", Path(__file__).with_name("inventory-gog.py"))
inventory = importlib.util.module_from_spec(spec)
spec.loader.exec_module(inventory)


class InventoryTests(unittest.TestCase):
    def test_empty_installation_reports_missing_prototype_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "missing exact paths"):
                inventory.inventory(Path(directory))

    def test_synthetic_installation_emits_metadata_without_payload(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative in inventory.REQUIRED:
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"SYNTHETIC-NOT-AN-ASSET")
            report = inventory.inventory(root)
            self.assertEqual(report["file_count"], 9)
            self.assertEqual(report["total_bytes"], 9 * 22)
            self.assertNotIn("SYNTHETIC", str(report))

    def test_symlink_outside_installation_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory)
            root = parent / "root"
            root.mkdir()
            (parent / "outside").write_text("synthetic")
            (root / "escape").symlink_to(parent / "outside")
            with self.assertRaisesRegex(ValueError, "escapes"):
                inventory.inventory(root)


if __name__ == "__main__":
    unittest.main()
