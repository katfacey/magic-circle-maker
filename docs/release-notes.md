# Magic Circle Maker 0.1.0-beta.1

The first native Rust desktop beta for 64-bit Windows.

Download **MagicCircleMaker-0.1.0-beta.1-windows-x64.zip**, extract it, and run **MagicCircleMaker.exe**. The ZIP includes the application and license notices. A standalone EXE is also attached. No Rust installation is required.

This release includes seeded alchemy/geometry/sigil generation, editable layers, Move/Rotate/Scale controls with canvas gizmos, custom artwork imports, bundled/system/custom fonts, project saving with embedded assets, undo/redo, Obsidian and Ivory themes, and aligned transparent PNG exports for each visible layer. Export sizes are 1024, 2048, and 4096 pixels.

The beta is unsigned. SVG artwork is rasterized on import; complex-script text layout, animation, multi-selection, and vector export are not implemented. See the README for limits and controls.

Validation includes Rust unit and GUI interaction tests, project/asset round trips, per-layer PNG and ZIP checks, and actual Windows native-window launch/screenshot checks in both themes. SHA-256 hashes are attached for the EXE and ZIP.

Created with assistance from OpenAI’s ChatGPT/Codex. This beta is provided as-is and may contain bugs. Please keep backups and report reproducible issues.
