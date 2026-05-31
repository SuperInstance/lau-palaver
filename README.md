# lau-palaver

Consensus protocol inspired by the West African palaver tree. Communities make decisions through extended discussion until EVERYONE agrees. Not majority vote. Not weighted vote. Consensus or we keep talking.

## The concept in 60 seconds

Under the palaver tree, there is no deadline. The group discusses until every voice is heard and every objection is addressed. This crate implements that:

- **Palaver sessions:** open discussions where every participant can speak
- **Consensus detection:** not "most people agree" — everyone agrees or we keep going
- **Objection tracking:** every dissent is recorded, not dismissed
- **Timeout tolerance:** palaver trees have no clocks — the process takes as long as it takes
- **Resolution:** when consensus is reached, it's durable because everyone was heard

The insight: consensus is slow, but consensus is stable. A decision everyone agreed to doesn't need enforcement.

## Quick start

```rust
use lau_palaver::{Palaver, Participant, Position};

let mut palaver = Palaver::new("deploy_to_production");

// Add participants with their positions
palaver.add_participant(Participant::new("hermes").with_position(Position::Agree));
palaver.add_participant(Participant::new("ensign").with_position(Position::Object("no rollback plan")));
palaver.add_participant(Participant::new("captain").with_position(Position::Neutral));

// Check consensus — not yet
assert!(!palaver.has_consensus());

// Address the objection
palaver.speak("hermes", "I'll add a rollback plan before the deploy");

// Update position
palaver.update_position("ensign", Position::Agree);

// Now check
if palaver.has_consensus() {
    println!("The palaver tree has spoken. Deploy.");
}
```

## Key types

| Type | What it is |
|------|-----------|
| `Palaver` | A consensus discussion session |
| `Participant` | A voice in the discussion with a name and position |
| `Position` | Agree, Object (with reason), or Neutral |
| `Resolution` | The final decision with full audit trail |

## Contributing

[Open an issue](https://github.com/SuperInstance/lau-palaver/issues) or PR.
