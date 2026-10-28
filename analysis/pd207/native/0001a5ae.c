void FUN_0001a5ae(byte param_1,int param_2)

{
  byte *pbVar1;
  byte local_c;
  byte local_b;
  byte local_a;
  byte local_9;

  if (param_2 == 1) {
    DAT_1fffb3b8 = DAT_1fffb3b8 | param_1;
    pbVar1 = &local_b;
    local_b = DAT_1fffb3b8;
  }
  else {
    if (param_2 == 0) {
      DAT_1fffae8c = DAT_1fffae8c | param_1;
      DAT_1fffb3b8 = DAT_1fffb3b8 & ~param_1;
      pbVar1 = &local_c;
      local_c = DAT_1fffae8c;
      goto LAB_0001a630;
    }
    DAT_1fffb3b8 = DAT_1fffb3b8 & ~param_1;
    pbVar1 = &local_a;
    local_a = DAT_1fffb3b8;
  }
  FUN_0001a736(2,pbVar1,1);
  if ((DAT_1fffae8c & param_1) == 0) {
    return;
  }
  DAT_1fffae8c = DAT_1fffae8c & ~param_1;
  pbVar1 = &local_9;
  local_9 = DAT_1fffae8c;
LAB_0001a630:
  FUN_0001a736(4,pbVar1,1);
  return;
}
