/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

void FUN_00040b58(int param_1)

{
  uint uVar1;

  uVar1 = param_1 - 1U & 0xff;
  if (uVar1 < 3) {
    _DAT_42a79718 = *(undefined4 *)(&DAT_00040b98 + uVar1 * 4);
    _DAT_42a79798 = *(undefined4 *)(&DAT_00040ba4 + uVar1 * 4);
  }
  else {
    _DAT_42a79718 = 0;
    _DAT_42a79798 = 0;
  }
  _DAT_40053bfc = 0xa500;
  return;
}
