void FUN_0001a63c(byte param_1,int param_2)

{
  byte *pbVar1;
  byte local_a;
  byte local_9;

  if (param_2 == 1) {
    DAT_1fffb3b9 = param_1 | DAT_1fffb3b9;
    pbVar1 = &local_a;
    local_a = DAT_1fffb3b9;
  }
  else {
    DAT_1fffb3b9 = DAT_1fffb3b9 & ~param_1;
    pbVar1 = &local_9;
    local_9 = DAT_1fffb3b9;
  }
  FUN_0001a736(3,pbVar1,1);
  return;
}
