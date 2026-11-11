# Application language

Import the context extension and use dotted keys:

```rust
use crate::lang::ActiveLang;

let label = cx.lang().get("common.cancel");
let message = cx.lang().get_with("titlebar.update_available", &[("version", "1.2.3")]);
```

`get` returns a GPUI `SharedString`, ready for labels, tooltips and text elements. Lookups use catalogs already loaded in memory. Nested TOML sections become dotted keys:

```toml
name = "Español"
code = "es"

[common]
cancel = "Cancelar"
```

Missing translations fall back to Spanish. Unknown keys return the key itself. Keys are exact: `cancel` does not match `common.cancel`. Named `{variables}` are replaced once; missing arguments remain visible.

The embedded catalogs are `es.toml` and `en.toml`. To add a language, create another TOML catalog and register its `include_str!` in `Lang::new`. Keep keys consistent between catalogs. The tests check the existing catalogs.

The selector in Settings → Appearance changes the active global language and saves `UIConfig.language` in the background. Existing settings default to Spanish. All views use these catalogs, including the dashboard, calendar, download cards, settings, validation messages and modals. Input placeholders update when the language changes without clearing entered values. Technical error details from the engine or operating system retain their original text.
