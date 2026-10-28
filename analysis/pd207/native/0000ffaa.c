/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

void FUN_0000ffaa(void)

{
  int iVar1;

  _DAT_40015480 = _DAT_40015480 & 0xffffaffe;
  iVar1 = FUN_0000ff98();
  if (iVar1 != 0) {
    FUN_0000f9c4();
    FUN_0004c8fa();
    FUN_0004c888(0x1fffb590,1);
    FUN_0004c91e();
    return;
  }
  return;
}
