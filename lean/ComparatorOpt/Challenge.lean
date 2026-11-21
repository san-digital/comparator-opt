/-!
# Pinned definitions and theorem statements

Every block between `@comparator:begin` and `@comparator:end` is hashed with SHA-256 and
recorded in `comparator.lock`, so `comparator-opt verify` fails if one changes.

The `ref*` definitions restate the Rust references in `src/kernels.rs`, and the `swar*` and
`fast*` definitions restate the Rust fast paths. Both were written out by hand. The proofs are
about these Lean copies. Nothing checks mechanically that the Rust code still matches them.
-/

namespace ComparatorOpt

-- @comparator:begin ascii_lower_challenge
def refAsciiLowerByte (b : UInt8) : UInt8 :=
  if 65 ≤ b ∧ b ≤ 90 then b + 32 else b

def swarAsciiLowerByte (b : UInt8) : UInt8 :=
  let low7 := b &&& 0x7f
  let geA := low7 + 0x3f
  let gtZ := low7 + 0x25
  let upperMsb := geA &&& (~~~gtZ) &&& (~~~b) &&& 0x80
  b ||| (upperMsb >>> 2)

def pack8 (b0 b1 b2 b3 b4 b5 b6 b7 : Nat) : Nat :=
  b0 + 256 * b1 + 65536 * b2 + 16777216 * b3 +
    4294967296 * b4 + 1099511627776 * b5 +
    281474976710656 * b6 + 72057594037927936 * b7

def AsciiLowerByteSound : Prop :=
  ∀ b : UInt8, swarAsciiLowerByte b = refAsciiLowerByte b

def SwarNoCarryAddSound : Prop :=
  ∀ (a0 a1 a2 a3 a4 a5 a6 a7 b0 b1 b2 b3 b4 b5 b6 b7 : Nat),
    pack8 a0 a1 a2 a3 a4 a5 a6 a7 + pack8 b0 b1 b2 b3 b4 b5 b6 b7 =
      pack8 (a0 + b0) (a1 + b1) (a2 + b2) (a3 + b3) (a4 + b4) (a5 + b5) (a6 + b6) (a7 + b7)
-- @comparator:end ascii_lower_challenge

-- @comparator:begin utf8_count_challenge
def refIsUtf8Lead (b : UInt8) : Bool :=
  (b &&& 0xc0) != 0x80

def swarUtf8LeadBit (b : UInt8) : UInt8 :=
  (((~~~b) ||| (b <<< 1)) &&& 0x80) >>> 7

def Utf8LeadBitSound : Prop :=
  ∀ b : UInt8, swarUtf8LeadBit b = (if refIsUtf8Lead b then 1 else 0)

def HorizontalByteSumSound : Prop :=
  ∀ (c0 c1 c2 c3 c4 c5 c6 c7 : Nat),
    c0 + c1 + c2 + c3 + c4 + c5 + c6 + c7 < 256 →
      (((pack8 c0 c1 c2 c3 c4 c5 c6 c7) * 0x0101010101010101) / 72057594037927936) % 256 =
        c0 + c1 + c2 + c3 + c4 + c5 + c6 + c7
-- @comparator:end utf8_count_challenge

-- @comparator:begin mersenne31_mod_challenge
def mersenne31 : Nat := 2147483647

def refMersenne31Valid (x : Nat) : Prop :=
  x < mersenne31 * mersenne31

def refMersenne31Mod (x : Nat) : Nat :=
  x % mersenne31

def fastMersenne31Mod (x : Nat) : Nat :=
  let t := x % 2147483648 + x / 2147483648
  let u := t % 2147483648 + t / 2147483648
  if u = mersenne31 then 0 else u

def Mersenne31FastSound : Prop :=
  ∀ x : Nat, refMersenne31Valid x → fastMersenne31Mod x = refMersenne31Mod x
-- @comparator:end mersenne31_mod_challenge

end ComparatorOpt
