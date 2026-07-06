# Modules and Packages

## Imports

```spider
use math
use helpers
use shop.cart
```

Stdlib imports always win. User imports load files:

- `use helpers` maps to `src/helpers.sp` in a project or sibling `helpers.sp`
  for scripts.
- `use shop.cart` maps to `src/shop/cart.sp`.
- `use package_name` can load `web_modules/package_name/src/lib.sp`.

## Visibility

Functions cross file boundaries only when declared `public`.

```spider
public fn greet(name: Text)
    say "Hello, {name}!"
```

Private functions remain local to their module.

## Side Effects

Only the entry file runs top-level code. Imported modules are side-effect-free.
If a module contains top-level executable statements, the project checker
reports E0246.

## Cycles

Import cycles are errors and include the cycle path.

## Limitations

- Types are project-global.
- Variant names share one global namespace.
- Transitive package dependencies are not resolved.
