/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

undefined4 FUN_0000fa8c(ushort *param_1,int param_2)

{
  _DAT_40053080 = param_1 + 1;
  _DAT_40053088 = _DAT_40053088 & 0xffff | (param_2 + -1) * 0x10000;
  _DAT_4005301c = _DAT_4005301c & 0xfffffffd | 2;
  _DAT_40015404 = (uint)*param_1;
  _DAT_40015400 = 0;
  _DAT_40015480 = _DAT_40015480 & 0xffffaffe | 1;
  return 0;
}
