/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

void FUN_0000fb46(int param_1,undefined4 param_2)

{
  undefined4 local_18;
  undefined4 local_14;

  local_18 = 0x80000;
  local_14 = param_2;
  FUN_0001dc14(&local_18);
  if (param_1 == 1) {
    *_DAT_1fffb3bc = *_DAT_1fffb3bc & 0xffffefff;
    _DAT_4004a000 = _DAT_4004a000 | 0x60;
  }
  else {
    _DAT_4004a000 = _DAT_4004a000 & 0xff9f;
    _DAT_40015480 = _DAT_40015480 & 0xffffeffe;
    *_DAT_1fffb3bc = *_DAT_1fffb3bc & 0xffffff3f | 0x40;
    *_DAT_1fffb3bc = *_DAT_1fffb3bc | 0xc0;
    *_DAT_1fffb3bc = *_DAT_1fffb3bc | 0x1000;
    _DAT_4005301c = _DAT_4005301c & 0xfffffffd;
  }
  return;
}
