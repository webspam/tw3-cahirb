# wcc_lite rsblob compiler rules

Verified against wcc_lite output. Evidence: `LR` = LightRewrite blob, `P:<name>` = probe.

## Encoding

| Rule | Evidence |
|---|---|
| Compressed ints set the sign bit for `value <= 0`, so zero is `0x80` | LR, root blob round trip |
| Definition strings: length stored negated (UTF-8) | LR round trip |
| `StringConst` operand: length stored positive, unsigned | LR round trip |
| Bytecode header `runtime_size` = sum of instruction runtime sizes | LR, all probes |
| Every skip offset = target address − address right after the skip field | P:enums |

## Source order

| Rule | Evidence |
|---|---|
| Per directory: files first, then subdirectories, recursively (depth first) | P:file_order |
| Names compared case-insensitively, upper-case folded (`_` sorts after letters) | P:file_order |
| Whole filename compared, including extension (`a-b.ws` before `a.ws`) | P:file_order_stem |
| Definitions within a file keep declaration order | P:file_order, LR |
| Classes: depth-first pre-order of the inheritance tree; roots (including classes with game parents) in source order; children in source order | P:class_order |
| Enums, structs, global functions, annotations: source order | LR |

## String table

| Rule | Evidence |
|---|---|
| Phase 1: function bodies compiled, strings interned on first use | P:literals, P:enums, LR |
| Phase 1 order: global functions, class methods (class order), `@replaceMethod` class, `@addMethod`, `@wrapMethod` | LR, P:annotations |
| Phase 1 position of `@replaceMethod` global relative to the others: unknown | |
| Phase 2: definitions serialized in blob order, interning remaining strings | P:minimal, P:class_order, P:function_flags |
| Import types interned on first use in phase 1 | P:enums |

## Flags

| Declaration | Flags |
|---|---|
| Global function | `0x2` static |
| `exec` global function | `0x102` |
| Method, no modifier or `public` | `0x10000` |
| `protected` / `private` method | `0x8000` / `0x4000` |
| `event` | `0x20` + visibility |
| `timer` | `0x400` + visibility |
| No body (`;`) | `0x200` + visibility |
| `abstract class` | `0x1` |
| `@addMethod` | `0x80000` + visibility (default public) |
| `@wrapMethod` | `0x100000` + `0x10000`, + target's `0x20` event / `0x400` timer; target visibility not copied |
| `@replaceMethod(Class)` | `0x40000` + `0x10000` |
| `@replaceMethod` (global, no parentheses) | `0x40000` + `0x2` |
| Class property | `0x20` + visibility (`0x40000` public default, `0x20000` protected, `0x10000` private) |
| `editable` / `saved` / `const` property | `+0x1` / `+0x8000` / `+0x2` |
| `@addField` | as class property, `class_name` = target class |
| Return / param / local | `0x40` named `__return` / `0x80` / `0x100` |

## Definitions

| Rule | Evidence |
|---|---|
| Enumerator without `= value` stored as `0x7FFFFFFF`; compiler still uses previous + 1 | P:enums |
| Empty body and undefined body both emit an empty bytecode (header only, `runtime_size=0`) | P:minimal, P:function_flags |
| Default values stored as strings (`"3"`), name `""` | P:function_flags |

## Codegen

| Source | Bytecode | Evidence |
|---|---|---|
| Non-empty body | ends with `Nop` | all |
| Statement | `Breakpoint(line)` first | all |
| `return e;` | `Return`, `Breakpoint(line)`, `e` | P:enums, LR |
| `x = e;` | `Assign`, target, `e` | P:literals |
| int `0` / `1` / other | `IntZero` / `IntOne` / `IntConst` | P:literals |
| `-n` | `FinalFunc Neg_Int32`, `n`, `ParamEnd` | P:literals |
| float literal | `FloatConst`, even `0.0` | P:literals |
| int literal into float | `IntToFloat`, int literal | P:literals |
| Script enum constant, all values fit `i8` | `ByteConst` (value as u8, `-1` → `255`) | P:enums, P:enum_size |
| Script enum constant, all values fit `i16` | `ShortConst` (`255`, `256`, `-200`) | P:enum_size |
| Script enum constant, otherwise | `IntConst` (`65535`, `65536`) | P:enum_size |
| Game-script enum (`InGameMenuActionType`) | same rule as mod enums (`ByteConst`) | P:enum_size |
| Engine enum (`ELightShadowCastingMode`, values 0..3) | `IntConst`, even `0` and `1` | LR |
| Enum `==` | `Equal_Int32_Int32` over `EnumToInt` on both sides | P:enums |
| `if` without `else` | `JumpIfFalse` past the body | P:enums |
| `switch` | `Switch(type, skip=expr size)`, expr, per case `SwitchLabel`, label expr, body | P:enums |
| `wrappedMethod(args)` | `VirtualFunc` named `@self` | P:function_flags |

## Native types

`ELightShadowCastingMode` is not declared in any game script; its values come from engine RTTI (listed in `tmp/enums`).
Its constants use `IntConst` although its values fit `i8`, so engine enums do not follow the script sizing rule. Only one engine enum observed.
