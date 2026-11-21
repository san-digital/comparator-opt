import ComparatorOpt.Proofs

/-!
# Binding each proof to its pinned statement

`Proofs.lean` is open to optimizers, so a theorem there could be restated at a weaker type
(for example `theorem ascii_lower_byte_sound : True := trivial`) and still pass an axiom
audit. Each `ComparatorOpt.Audit` theorem below restates one pinned `Challenge.lean` target
and is proved by the matching `Proofs.lean` theorem, so this file only compiles if that
theorem really proves the target. Its axiom footprint, audited through `lean/axioms.txt`,
also covers anything the elaborator inserts to make the term fit, such as a coercion.

Every name below is written from `_root_` so that declarations added to `Proofs.lean`
cannot shadow the pinned targets. This block is pinned in `comparator.lock`.
-/

-- @comparator:begin lean_statement_binding
theorem ComparatorOpt.Audit.ascii_lower_byte_sound :
    _root_.ComparatorOpt.AsciiLowerByteSound :=
  _root_.ComparatorOpt.ascii_lower_byte_sound

theorem ComparatorOpt.Audit.swar_no_carry_add_sound :
    _root_.ComparatorOpt.SwarNoCarryAddSound :=
  _root_.ComparatorOpt.swar_no_carry_add_sound

theorem ComparatorOpt.Audit.utf8_lead_bit_sound :
    _root_.ComparatorOpt.Utf8LeadBitSound :=
  _root_.ComparatorOpt.utf8_lead_bit_sound

theorem ComparatorOpt.Audit.horizontal_byte_sum_sound :
    _root_.ComparatorOpt.HorizontalByteSumSound :=
  _root_.ComparatorOpt.horizontal_byte_sum_sound

theorem ComparatorOpt.Audit.mersenne31_fast_sound :
    _root_.ComparatorOpt.Mersenne31FastSound :=
  _root_.ComparatorOpt.mersenne31_fast_sound
-- @comparator:end lean_statement_binding
