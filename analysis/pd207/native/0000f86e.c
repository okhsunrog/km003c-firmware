/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

void FUN_0000f86e(int param_1,int param_2,int param_3)

{
  ushort uVar1;
  undefined4 local_10;
  int local_c;

  _DAT_400530c0 = param_2 * 4 + 0x40015440;
  if (param_1 == 8) {
    uVar1 = 0x800;
  }
  else if (param_1 == 2) {
    uVar1 = 0x200;
  }
  else {
    if (param_1 != 1) goto LAB_0000f8b2;
    uVar1 = 0x100;
  }
  _DAT_4004a002 = uVar1 | _DAT_4004a002 & 0xf0ff;
LAB_0000f8b2:
  if (param_3 != 0) {
    local_10 = 0x80000;
    local_c = param_3;
    FUN_0001dc14(&local_10);
  }
  return;
}
