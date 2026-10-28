/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

void FUN_0000f976(undefined4 param_1,int param_2,int param_3)

{
  undefined4 local_20;
  int local_1c;
  undefined4 local_18;
  undefined4 local_14;

  local_14 = 0;
  local_18 = 0;
  FUN_0000955c(0,param_1,&local_18);
  FUN_00009678(0,param_1,4,0);
  _DAT_1fffb3bc = param_2 * 4 + 0x40015540;
  if (param_3 != 0) {
    local_20 = 0x80000;
    local_1c = param_3;
    FUN_0001dc14(&local_20);
  }
  return;
}
