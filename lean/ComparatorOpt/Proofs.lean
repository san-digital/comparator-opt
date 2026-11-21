import ComparatorOpt.Challenge

/-!
# Machine-Checked Equivalence Proofs for Superoptimized Kernels

Discharges every theorem target in `ComparatorOpt.Challenge` in stock Lean 4 (`Init`)
with zero Mathlib dependency, zero `sorryAx`, and zero `native_decide` (`Lean.ofReduceBool`).
-/

namespace ComparatorOpt

set_option maxRecDepth 4096 in
private theorem ascii_lower_fin : ∀ n : Nat, n < 256 →
    swarAsciiLowerByte (UInt8.ofNat n) = refAsciiLowerByte (UInt8.ofNat n) := by
  decide

theorem ascii_lower_byte_sound : AsciiLowerByteSound := by
  intro b
  have hlt : b.toNat < 256 := b.toBitVec.isLt
  have hb : UInt8.ofNat b.toNat = b := UInt8.ofNat_toNat
  rw [← hb]
  exact ascii_lower_fin b.toNat hlt

theorem swar_no_carry_add_sound : SwarNoCarryAddSound := by
  intro a0 a1 a2 a3 a4 a5 a6 a7 b0 b1 b2 b3 b4 b5 b6 b7
  unfold pack8
  omega

set_option maxRecDepth 4096 in
private theorem utf8_lead_fin : ∀ n : Nat, n < 256 →
    swarUtf8LeadBit (UInt8.ofNat n) = (if refIsUtf8Lead (UInt8.ofNat n) then 1 else 0) := by
  decide

theorem utf8_lead_bit_sound : Utf8LeadBitSound := by
  intro b
  have hlt : b.toNat < 256 := b.toBitVec.isLt
  have hb : UInt8.ofNat b.toNat = b := UInt8.ofNat_toNat
  rw [← hb]
  exact utf8_lead_fin b.toNat hlt

theorem horizontal_byte_sum_sound : HorizontalByteSumSound := by
  intro c0 c1 c2 c3 c4 c5 c6 c7 hsum
  unfold pack8
  let s := c0 + c1 + c2 + c3 + c4 + c5 + c6 + c7
  let q_hi :=
    c1 + 257 * c2 + 65793 * c3 + 16843009 * c4 +
      4311810305 * c5 + 1103823438081 * c6 + 282578800148737 * c7
  let r_lo :=
    282578800148737 * c0 + 282578800148736 * c1 + 282578800148480 * c2 +
      282578800082944 * c3 + 282578783305728 * c4 + 282574488338432 * c5 +
      281474976710656 * c6
  have hdecomp :
      (c0 + 256 * c1 + 65536 * c2 + 16777216 * c3 +
        4294967296 * c4 + 1099511627776 * c5 +
        281474976710656 * c6 + 72057594037927936 * c7) * 0x0101010101010101 =
        r_lo + (s + 256 * q_hi) * 72057594037927936 := by
    dsimp [s, q_hi, r_lo]
    omega
  have hr_lt : r_lo < 72057594037927936 := by
    dsimp [r_lo]
    omega
  rw [hdecomp, Nat.add_mul_div_right _ _ (by decide), Nat.div_eq_of_lt hr_lt, Nat.zero_add]
  rw [Nat.add_mul_mod_self_left]
  exact Nat.mod_eq_of_lt hsum

theorem mersenne31_fast_sound : Mersenne31FastSound := by
  intro x hvalid
  unfold refMersenne31Valid mersenne31 at hvalid
  unfold fastMersenne31Mod refMersenne31Mod mersenne31
  dsimp only
  let q1 := x / 2147483648
  let r1 := x % 2147483648
  let t := r1 + q1
  let q2 := t / 2147483648
  let r2 := t % 2147483648
  let u := r2 + q2
  have hx1 : x = 2147483648 * q1 + r1 := (Nat.div_add_mod x 2147483648).symm
  have hr1 : r1 < 2147483648 := Nat.mod_lt x (by decide)
  have ht1 : t = 2147483648 * q2 + r2 := (Nat.div_add_mod t 2147483648).symm
  have hr2 : r2 < 2147483648 := Nat.mod_lt t (by decide)
  have hx_cong : x = u + (q1 + q2) * 2147483647 := by
    dsimp [t, u] at ht1 ⊢
    omega
  have hu_le : u ≤ 2147483647 := by
    dsimp [t, u] at ht1 ⊢
    omega
  change (if u = 2147483647 then 0 else u) = x % 2147483647
  by_cases hu_eq : u = 2147483647
  · simp only [hu_eq, ↓reduceIte]
    have hx_mul : x = (q1 + q2 + 1) * 2147483647 := by omega
    rw [hx_mul, Nat.mul_mod_left]
  · simp only [hu_eq, ↓reduceIte]
    have hu_lt : u < 2147483647 := by omega
    rw [hx_cong, Nat.add_mul_mod_self_right, Nat.mod_eq_of_lt hu_lt]

end ComparatorOpt
