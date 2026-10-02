# Proprietary data policy

Civilization III Complete data must be supplied from a user's own installation. The GOG installer and all extracted executables, artwork, sound, fonts, rules/scenario data, and other proprietary payloads remain local. They must not enter source control, uploaded documents, CI caches/artifacts, APKs, or releases.

Use `installer/` for the original input and `local-data/gog/` for inspection output. Both are ignored. Do not execute or modify the installer. The inspection command is `innoextract --extract --include app --output-dir local-data/gog installer/setup_civilization3_complete_2.0.0.7.exe`. This extracts files without running Windows code.

Public inventories contain relative paths, byte counts, classifications, and technical metadata only. The initial Android APK contains open-source Lua rules from FreeC3, not Civilization III data. Public CI runs only synthetic-data tests.

The path-policy script is an additional check, not a guarantee that arbitrary renamed content is safe. Review every newly included file and inspect APK ZIP entries. Ignore rules do not remove a previously tracked file; stop if proprietary data is discovered in Git history.
