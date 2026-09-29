# Conventions

Binding on every contributor, human or agent. If a convention here blocks you, say so in your
report. Do not quietly deviate.

---

## 0. Design style

The rules below are the general shape of the code. Everything after this section is a specific
application of them, so when a later rule seems arbitrary, this is why it exists.

### Prefer pure functions

A function that takes values and returns values can be tested with a table, reasoned about in
isolation, and reused from a context its author never imagined. One that reaches for a socket,
a clock or a global cannot.

Push effects to the edges. In palmrest the core (`sensor`, `touch`, `stamp`, `feel`,
`gestures`, `tracker`, `action`, `protocol`, `config`) takes bytes and instants and returns
values; only `daemon` owns threads, sockets, devices and clocks. When the core seems to need an
effect, the effect belongs to the caller: `Tracker::step` returns `Out`s rather than writing to
a device.

Effects belong in the signature. `&mut self`, `Result`, or living in a module permitted to do
I/O are how a reader sees that something happens; a function that looks pure and is not is
worse than one that is honestly imperative.

### Separate data from logic

Data types describe what is true. Functions describe what follows from it. Keep them apart.

A struct's own `impl` should hold construction, derivation and accessors, the things that are
*about* that value. Anything that coordinates several values, or makes a decision, reads better
as a free function or as an `impl` on the type that owns the decision.

Avoid the type that grows methods because it was convenient rather than because they belong.
If half a struct's methods never touch half its fields, it is two types.

### Make illegal states unrepresentable

Encode invariants in types rather than checking them at runtime and hoping every caller
remembers.

- Newtype every identifier and every unit (`Um`, `Ms`, `Fraction`, `GestureId`). A newtype
  costs nothing at runtime and makes an argument-order mistake a compile error.
- No stringly-typed fields where a closed set exists.
- No `bool` in state (see §4). No `bool` parameters either: a call reading `touches(true)`
  tells the reader nothing, while `touches(ActiveTouch::Down)` tells them everything.
- Prefer an empty collection to `Option<Vec<T>>`. There is one way to say "none" and it needs
  no unwrapping.

### Parse, don't validate

Convert untrusted input into a type that cannot be wrong, once, at the boundary; then the rest
of the program needs no defensive checks. `decode` turns device bytes into a `Report`. A
validation function that returns `bool` and leaves the caller holding the same loose type has
moved the problem, not solved it.

A fallible constructor returns `Result`. A constructor that cannot fail returns `Self`.

### Traits are seams, not decoration

A trait earns its place when two implementations are genuinely swapped at that boundary:
`Sensor` (`MagicMouseHid`, `EvdevMultitouch`), `Emitter` (uinput and `RecordingEmitter`). A
trait with one implementation is indirection with no payoff.

Prefer an enum to a trait for closed vocabulary, so that adding a variant produces a list of
compile errors naming every site that must be updated. A `dyn Trait` hides that list.

Implement the standard traits when the semantics genuinely hold, and derive rather than
hand-write unless a behaviour must differ. `Display` is for humans, `Debug` is for developers,
and neither is a serialization format; that is `serde`'s job.

### Errors are values

An error type exists so a caller can *act*, not so a string can be logged. Reserve panics for
broken invariants inside our own code, and document them as caller contracts. Malformed input
is never a panic: the bytes came from a device, a file or a socket.

### Borrowing from other projects

This workspace is `MIT OR Apache-2.0`. The licence of what you read determines what you may do
with it:

- **permissive** (MIT, Apache-2.0, BSD, 0BSD): code may be copied, with attribution.
- **weak copyleft** (MPL, LGPL): link freely, but MPL's unit of copyleft is the *file*: paste
  one function and that file becomes MPL.
- **strong copyleft** (GPL, AGPL, EUPL): read for facts, never paste.

**The rule for reading is note-and-close.** Read the source, reduce what you learned to a
sentence about observable behaviour, close the file, implement from the sentence. Never put
GPL, AGPL or MPL source into a model's context and ask for an equivalent. Cite the behaviour,
not the source file.

### Composition over accumulation

Prefer iterator chains and small combinators where they read more clearly than a loop, and a
plain loop where they do not. Prefer returning a new value to mutating an argument. Where a
mutable accumulator really is the clearest expression, keep it local to the function.

Keep functions short enough to hold in your head. A long function is usually several functions
that have not been named yet.

## 1. Derives

Public types derive, in this order, what applies:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
```

- `Copy`: only for types that are one word or smaller (IDs, fieldless enums).
- `Hash`: every ID newtype, and anything used as a map key.
- `PartialOrd, Ord`: only where a total order genuinely means something.
- `Default`: only where a default is genuinely meaningful.

## 2. Serde

- **Enums with data: adjacently tagged**, `#[serde(tag = "kind", content = "v", rename_all =
  "snake_case")]`. Internal tagging cannot represent newtype variants over non-map types.
- **Fieldless enums:** `#[serde(rename_all = "snake_case")]`.
- **Structs:** field names as written, `snake_case`.

## 3. Errors

`thiserror` enums, one per concern. No `unwrap()` or `expect()` in non-test code, except where
a comment on the same line proves the invariant.

## 4. Naming

- Enums read as values at the use site: `Rejection::On`, not `Rejection::IsOn`.
- No `bool` fields in state. Predicate **returns** are `bool`.
- No `get_` prefix.
- Name a value for what it does, not for where it came from.

## 5. Modules and files

One concept per file; a file over ~400 lines wants splitting. Public items carry a doc comment
saying what they mean, not what they are.

## 6. Tests

- **Unit tests** in `#[cfg(test)] mod tests` beside the code, for private behaviour.
- **Integration tests** in `crates/<crate>/tests/<topic>.rs`, for the public surface.
- Table-driven wherever there is more than one case: a `const CASES: &[(Input, Expected)]`
  and one loop. A failure must name the case.
- Tests never touch the real system: no input device is created, no real config directory or
  daemon is read (see README, "Safety rules").
- **Assert the difference the action makes**, not a state the action happens to be compatible
  with. The tell is an assertion that could be moved above the line that does the work and still
  pass.
- **Substrings are not tokens.** When the thing being matched has a grammar (an id, a key, a
  word), match it at its boundaries: `HID_ID` is parsed as `bus:vendor:product`, not searched.

## 7. Verification

Before reporting work complete, all of these pass:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check licenses
```

Before reporting that something does not work, run it the way its user would, with nothing of
yours around it. Report honestly: a failing test reported as passing costs more than the bug.

## 8. Repository rules

- Small structs; functional style where it fits; data separate from logic; type-driven:
  newtypes and enums carry meaning, traits only at real seams.
- No `bool` anywhere in a public signature or a settings key: two-state enums with named
  variants.
- No `unsafe` outside `sys.rs`, each block with a SAFETY comment.
- Dependencies are settled first: the pinned block in quire's `docs/workspace-deps.toml` is
  copied verbatim into every workspace; a new dependency is a change to that file first.
- Every value the design docs mark "proposed" is read from a settings key
  (`design/22-SETTINGS.md`) with that default; never hard-coded.
