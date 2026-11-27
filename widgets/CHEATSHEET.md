# Groh Widget Cheatsheet

For writing or reviewing Groh widgets (`.groh` files — Rhai scripts that return a `Node` tree).
Derived from [Rhai's own cheatsheet](https://raw.githubusercontent.com/rhaiscript/rhai/refs/heads/main/RHAI-CHEATSHEET.md);
Groh-specific sandbox notes are marked ◆. Full reference: [_The Rhai Book_](https://rhai.rs/book/ref).

## JS → Rhai deltas (read first)

| JavaScript habit | Rhai reality |
|---|---|
| `{a: 1}` object literal | `#{a: 1}` (object map) |
| `null` / `undefined` | single value `()`; absent map key reads as `()` |
| truthiness (`if (x)`) | conditions must be `bool` |
| `===` / `==` coercion | only `==`; no coercion; `1` (`i64`) and `1.0` (`f64`) are distinct types |
| objects/arrays are references | **no references exist**: values are cloned on assign and on call; mutate via the variable path |
| `function f(){}` sees globals/outer scope | `fn` cannot see outer variables; closures `\|x\| ...` capture (shared) |
| `arr.length`, `.forEach`, `.includes` | `arr.len()`, `for` loop, `.contains()` |
| `console.log` | `print(x)` / `debug(x)` — ◆ not available in Groh (see below) |
| `Math.floor(x)` | `x.floor()`; `PI()`; `x.sin()` etc. |
| `for (let i=0;i<n;i++)` | `for i in 0..n` (exclusive) / `0..=n` (inclusive) |
| `if (c) stmt` | `if c { ... }` braces mandatory, no parentheses |
| `return` required | last statement is always the value of the block (not just functions), with or without trailing `;` (different from Rust) |
| Blocks <> expressions | every block is an expression (the last statement is the value) |
| `throw`/`try`/`catch (e)` | same; `throw` takes any value |
| `class`, `async`, `await`, `new` | none; use maps + closures, or host-registered types |
| `import x from` | `import "name" as x; x::f()` |
| `'a'` is a string | `'a'` is a `char`; strings use `"..."` or `` `...${x}...` `` |
| `s.length`, UTF-16 indexing | `s.len()` counts chars; `s[i]` is a `char` (different from Rust where strings are UTF-8 encoded bytes) |
| `x = a ? b : c` | `x = if a { b } else { c }` |

When unsure whether a method exists, check the host's registered functions instead of guessing JS names.

## Values

```rhai
let count = 0;                  // mutable, dynamically typed
const LIMIT = 10;
let u;                          // ()
let items = [10, 20, 30,];      // trailing comma ok; items[-1] == 30
let opts = #{mode: "fast", "odd-key": true};
opts.mode = "safe";
opts["odd-key"];                // brackets for non-identifier keys
"mode" in opts;                 // key test
opts.timeout ?? 30;             // ?? defaults when left is ()
opts?.a?.b;                     // ?. short-circuits on ()
x = if a { b } else { c };      // if statement used as expression, similar to x = a ? b : c;
```

- Integer literals: decimal, `0b`, `0o`, `0x`, `_` separators. Default int is `i64`.
- Out-of-range array reads are errors. Unsupported operations are errors, not coercions.
- Blocks, `if`, `switch`, `while`, `loop`, `for` are expressions; `break value` yields from a loop.
- Mismatched non-numeric `==` is `false`; mixed numeric types may fail: convert with `to_int()`/`to_float()`.
- `&&`/`||` short-circuit; single `&`/`|` on booleans evaluate both sides.

## Value semantics (◆ Groh-relevant)

```rhai
let a = [1, 2];
let b = a;                      // clone
b[0] = 99;                      // a unchanged
fn f(x) { x[0] = 99; }
f(a);                           // a unchanged

a[0] = 7;                       // edits a
let user = #{s: #{theme: "light"}};
user.s.theme = "dark";          // edits nested property in place
let s = user.s;                 // clones
```

- Rhai has no reference type: scripts cannot hold, pass, or return a reference to a value.
- Property/index paths rooted at a variable mutate in place (built-in arrays, maps only).
- ◆ String methods such as `trim()` **mutate in place and return `()`** — call them as statements, never assign the result:
  ```rhai
  key.trim();        // correct: key is now trimmed
  key = key.trim();  // wrong: key becomes ()
  ```
- ◆ Missing data from host calls arrives as `()`, not `null`:
  ```rhai
  if c.mem_usage == () { "—" } else { ... }
  ```

## Control flow

```rhai
for item in items { if item == 20 { continue; } if item > 30 { break; } }
for (item, i) in items { }      // with index
let r = if ok { "go" } else { "wait" };   // omitted else yields ()
let n = 0; while n < 3 { n += 1; }
loop { break; }
```

`switch` exists (`switch x { 1 => ..., 2 | 3 => ..., _ => ... }`); see the reference for ranges and guards.

◆ Expression complexity limit: a single expression that is too complex fails to compile with
`Expression exceeds maximum complexity`. In practice this bites on long if-else chains and on
interpolations containing method chains. Split long chains into helper functions, and build
URLs or long strings with `+` concatenation:

```rhai
// This can fail:
let url = `https://example.com/?lat=${config.lat}&lon=${config.lon}`;
// This works:
let url = "https://example.com/?lat=" + config.lat + "&lon=" + config.lon;
```

## Functions and closures

```rhai
fn add(a, b) { a + b }                  // global-level only, no nesting, no capture
let offset = 2;
let f = |n| n + offset;                 // closure: captures SHARED variables
f.call(3);                              // call closures/pointers with .call()
let g = add;                            // fn pointer (same as Fn("add"))
g.call(1, 2);
```

- Function pointers store only a name; they cannot name functions inside modules.
- ◆ Widgets normally don't need closures or fn pointers — plain `fn` helpers are enough.

## Strings

`"escaped"`, `` `interpolated ${x}, no escapes` ``, `#"raw \ string"#`, `'c'` (char).

◆ Keep interpolations simple (`${rounded}`, `${name}`); put method chains in a helper first.

## ◆ Groh sandbox: what exists

A widget script runs in a restricted engine. Only these functions exist:

**Node constructors** (return `Node` values; the script's final value must be a `Node`):

```rhai
text(value)                       // value: string/int/float/bool/() — () becomes ""
text(value, #{size: "small", color: "cyan", bold: true})
box([child, ...])                 // framed container
row([child, ...])                 // gap defaults to 8; row(children, #{gap: 4})
column([child, ...])              // same options as row
separator()                       // separator(#{kind: "dashed"}) — kinds: dotted (default), dashed, solid; add orientation: "vertical" for a vertical line
bar(percent)                      // clamped to 0..=100; bar(p, #{color: "light-red"})
status_dot("running")             // tones: running, warn, stopped, other
link(label, href)                 // only http(s):// and /-relative hrefs render
image(src) / image(src, alt)
icon(src) / icon(src, alt)
window(title, [child, ...])       // modal details dialog
```

Sizes: `small`, `normal` (default), `large`. Colors: the 17 palette names
(`foreground`, `black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`,
`light-black`, `light-red`, `light-green`, `light-yellow`, `light-blue`, `light-magenta`,
`light-cyan`, `light-white`).

The returned tree is validated: at most 2000 nodes, depth at most 64. The engine also caps
operations (1,000,000), string/array/map sizes, and wall-clock time per run
(`--script-timeout`, default 5s), so infinite loops and runaway growth error out.

**Host capabilities** — only the ones listed in `META.needs` are registered:

| `needs` entry | Available calls |
|---|---|
| `net:http` | `http::get(url)` → string; `http::get_json(url)` → parsed JSON value. ◆ Every request and redirect hop is checked against `META.hosts` (exact hostname match). |
| `docker:containers` | `docker::containers()` → array of container maps (`id`, `name`, `state`, `detail`, `image`, `description`, `url`, `icon`, `category`, `started_epoch`, `cpu_percent`, `mem_usage`, `mem_limit`, `children`). |
| `time:now` | `time::now()` → Unix epoch seconds (`i64`). |

Nothing else is registered: no `print`, no filesystem, no shell, no environment access.

## ◆ META and config

Every widget starts with a `const META` literal header (it is extracted, not executed):

```rhai
const META = #{
    title: "Weather",        // required, non-empty
    interval: "10m",         // required: suffix s, m, or h (e.g. "30s", "10m", "1h")
    size: "small",           // required: small | normal | large (placement can override)
    needs: ["net:http"],     // optional: net:http | docker:containers | time:now
    hosts: ["api.open-meteo.com"],  // optional: allowlist for net:http
    config: #{               // optional: config schema, see below
        lat: #{ type: "number", fallback: 51.2194, help: "Latitude" },
    },
};
```

Config key types: `string`, `number`, `bool`, `list` (list of strings). Each key may set:

- `required: true` — the key must be provided in `groh.toml`; `required` and `fallback` cannot combine.
- `fallback: <value>` — default when not provided; must match the declared type.
- `secret: true` — value is redacted from error messages.
- `help: "…"` — documentation only; ignored by the host.

Values arrive in the script as the `config` variable (a map): `config.lat`.

## ◆ Registering a new widget

1. **Create the script** at `widgets/<Name>.groh`, starting with `META` and ending with a `Node` expression. Use the template below.
2. **Place it** in `groh.toml`:
   ```toml
   [[page.columns]]
   [[page.columns.widgets]]
   id = "weather"
   source = "widgets/Weather.groh"
   size = "small"          # optional: overrides META.size
   show_summary = true     # optional, default true
   ```
   `source` resolution: an absolute path is used as-is; a path starting with `widgets/` is
   relative to the working directory; anything else is relative to `--widgets-dir`
   (default `./widgets`).
3. **Optional config values** go in a table named after the placement `id`:
   ```toml
   [widgets.weather.config]
   lat = 52.37
   lon = 4.90
   ```
   Unknown keys and missing required keys fail startup with a clear error.
4. **Restart** — config and scripts are loaded once at startup; there is no hot reload.

To add a new *host capability* (not just a widget): add a variant to the `Need` enum in
`src/script/mod.rs`, implement it as a module under `src/script/host/`, register it in
`build_engine` next to the existing three, and document it in the table above.

## ◆ Template

```rhai
const META = #{
    title: "My Widget",
    interval: "1m",
    size: "small",
    needs: [],
    hosts: [],
    config: #{},
};

let value = 42;

column([
    text("Hello", #{ bold: true }),
    text(`Value: ${value}`, #{ color: "cyan", size: "small" }),
])
```
