/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

void FUN_00040ae0(int param_1)

{
  if (param_1 == 2) {
    FUN_0001a5ae(8,0);
    return;
  }
  if (param_1 == 1) {
    _DAT_42a70298 = 0;
    return;
  }
  _DAT_42a70298 = 0;
  FUN_0001a5ae(8,0);
  return;
}
