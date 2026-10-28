/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

void FUN_00040bb0(int param_1)

{
  if (param_1 == 2) {
    _DAT_42a79798 = 0;
    _DAT_40053bfc = 0xa500;
    _DAT_42a704b8 = 1;
    _DAT_42a704f8 = 1;
    FUN_0001a63c(0x40,1);
    return;
  }
  if (param_1 == 1) {
    _DAT_42a79718 = 0;
    _DAT_40053bfc = 0xa500;
    _DAT_42a704bc = 1;
    _DAT_42a704fc = 1;
    FUN_0001a5ae(8,0);
    return;
  }
  FUN_0001a5ae(8,0);
  FUN_0001a63c(0x40,1);
  _DAT_42a704f8 = 0;
  _DAT_42a704fc = 0;
  return;
}
