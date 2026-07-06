# Bytecode Reference

Silk bytecode is register based. Each function prototype has a fixed register
count, bytecode instructions, and a constant table.

## Program

- `protos`: function prototypes.
- `entry`: script entry prototype.
- `main`: optional zero-arity `main`.
- `n_globals`: global slot count.
- `tests`: named test prototypes.

## Instructions

| Instruction | Operands | Purpose |
| --- | --- | --- |
| `LoadConst` | dst, const | Load constant. |
| `LoadUnit` | dst | Load `nothing`. |
| `Move` | dst, src | Copy value. |
| `LoadGlobal` | dst, slot | Read global. |
| `StoreGlobal` | slot, src | Write global. |
| `Bin` | op, dst, a, b | Binary operation. |
| `Neg` | dst, src | Numeric negation. |
| `Not` | dst, src | Boolean negation. |
| `Jump` | target | Unconditional jump. |
| `JumpIfFalse` | cond, target | Conditional branch. |
| `JumpIfTrue` | cond, target | Conditional branch. |
| `Call` | dst, proto, args | Call known function. |
| `CallValue` | dst, callee, args | Call function value. |
| `CallMethod` | dst, recv, name, args | Call method. |
| `CallModule` | dst, module, name, args | Call module function. |
| `ModuleConst` | dst, module, name | Load module constant. |
| `MakeList` | dst, items | Construct list. |
| `MakeMap` | dst, pairs | Construct map. |
| `MakeRange` | dst, lo, hi | Construct range. |
| `Index` | dst, base, index | Read index. |
| `IndexSet` | base, index, value | Write index. |
| `GetField` | dst, base, field | Read record field. |
| `SetField` | base, field, value | Write record field. |
| `MakeRecord` | dst, shape, fields | Construct record. |
| `MakeVariant` | dst, tag, fields | Construct choice value. |
| `TestTag` | dst, value, tag | Test variant tag. |
| `GetVariantField` | dst, value, pos | Read variant payload. |
| `TryUnwrap` | dst, value, fail_target | Unwrap `Ok`/`Some` or branch. |
| `NoneToFail` | reg | Convert `None` propagation to `Fail`. |
| `IterNew` | dst, iterable | Create iterator. |
| `IterNext` | item, iter, done_target | Advance iterator. |
| `Concat` | dst, parts | String interpolation concatenation. |
| `Say` | src | Print value. |
| `Ask` | dst, prompt | Prompt and read text. |
| `Ret` | src | Return value. |
| `RetUnit` | none | Return `nothing`. |

## Performance Notes

Instruction operands currently use ordinary Rust enum variants and vectors.
Packing and optimization are intentionally deferred to M8.
