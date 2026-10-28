/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

undefined4 FUN_0000fa02(undefined4 param_1,int param_2,undefined4 param_3)

{
  _DAT_400530c4 = param_1;
  _DAT_400530c8 = _DAT_400530c8 & 0xffff | param_2 << 0x10;
  _DAT_4005301c = _DAT_4005301c & 0xfffffffb | 4;
  _DAT_40052008 = 0;
  _DAT_40015404 = param_3;
  _DAT_40015480 = _DAT_40015480 & 0xffffbffe | 0x1000;
  return 0;
}
