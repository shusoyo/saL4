# Commit Style

Use the following format for commit messages:

```text
<symbol>. <part>: <message>
```

## Symbols

- `+.` for adding new content or new functionality
- `~.` for modifying existing content or behavior
- `-.` for removing content

## Parts

Use the main component or area affected by the change, for example:

- `boot`
- `main`
- `cap`
- `config`
- `doc`
- `utils`

If a change spans multiple closely related parts, they may be combined with `/`:

```text
~. boot/config: rename early boot memory constant
```

## Examples

```text
+. cap: add frame capability
~. main: refine boot flow
-. doc: remove outdated notes
~. boot/config: clarify boot info layout
```
