
/* WARNING: Globals starting with '_' overlap smaller symbols at the same address */

void adc_measure_task(void)

{
  uint *puVar1;
  longlong lVar2;
  ulonglong uVar3;
  bool bVar4;
  undefined2 uVar5;
  uint *puVar6;
  int iVar7;
  int iVar8;
  int *piVar9;
  int iVar10;
  undefined2 *puVar11;
  short *psVar12;
  ushort uVar13;
  uint uVar14;
  undefined1 *puVar15;
  uint uVar16;
  uint uVar17;
  uint uVar18;
  char unaff_r8;
  int iVar19;
  bool bVar20;
  char cStack_54;
  undefined1 *apuStack_50 [2];
  uint auStack_48 [4];
  ushort *puStack_38;
  ushort *puStack_2c;
  uint uStack_18;
  int aiStack_14 [5];
  
  uVar16 = (uint)uRam4004000c;
  FUN_000052d0(0);
  if (uVar16 == 0) {
    uVar18 = 1;
    uVar16 = 1;
    apuStack_50[0] = &DAT_40040070;
  }
  else {
    puVar15 = (undefined1 *)0x40040050;
    uVar13 = 0;
    uVar18 = 0;
    do {
      if ((uVar16 & 1) != 0) {
        uVar17 = (uint)uVar13;
        uVar14 = (uint)uVar13;
        uVar13 = uVar13 + 1;
        apuStack_50[uVar17 * 2 + uVar14] = puVar15;
      }
      if (uVar16 < 2) break;
      uVar16 = uVar16 >> 1;
      bVar20 = uVar18 < 3;
      uVar18 = uVar18 + 1;
      puVar15 = puVar15 + 2;
    } while (bVar20);
    bVar20 = true;
    uVar16 = uVar13 + 1;
    uVar18 = uVar16 & 0xffff;
    apuStack_50[(uint)uVar13 * 2 + (uint)uVar13] = &DAT_40040070;
    if (uVar18 != uVar16) {
      uVar18 = 0;
      bVar4 = false;
      goto LAB_0001def0;
    }
  }
  FUN_000043fe(aiStack_14,uVar18 << 2);
  puVar6 = auStack_48;
  uVar14 = uVar18;
  do {
    uVar14 = uVar14 - 1;
    *puVar6 = 0;
    puVar6 = puVar6 + 3;
  } while (uVar14 != 0);
  bVar4 = false;
  bVar20 = false;
  if (1 < (uVar16 & 0xffff)) {
    bVar4 = true;
  }
LAB_0001def0:
  _DAT_1fffacdc = FUN_0004ca98(0x500);
  FUN_00017e34(1);
  iVar19 = 0;
  do {
    do {
      if (sRam40040004 != 0) {
        DAT_40040000 = 1;
      }
      if ((_DAT_1fffad3e & 3) == 3) {
        FUN_0004cffc(1);
        iVar7 = FUN_00006e54();
        iVar8 = FUN_00006d78();
        FUN_000484ce();
        piVar9 = (int *)FUN_000483ca();
        iVar10 = FUN_000483d6();
        iVar10 = (int)((ulonglong)((longlong)iVar10 * -0x10624dd3) >> 0x20);
        lVar2 = (longlong)((*piVar9 / 100) * (((iVar10 >> 6) - (iVar10 >> 0x1f)) + iVar8 / 1000)) *
                0x68db8bad;
        _DAT_1fffad5a = (short)(uint)((ulonglong)lVar2 >> 0x2c) - (short)(lVar2 >> 0x3f);
        _DAT_1fffad58 = (undefined2)((uint)(iVar7 * 100) >> 9);
        FUN_00017b1c(iVar7 * 0xc35 >> 4,iVar8);
      }
      else {
        FUN_0004cffc(2);
        iVar7 = FUN_00006e54();
        iVar8 = FUN_00006d78();
        FUN_000484ce();
        piVar9 = (int *)FUN_000483ca();
        iVar10 = FUN_000483d6();
        iVar10 = (int)((ulonglong)((longlong)iVar10 * -0x10624dd3) >> 0x20);
        lVar2 = (longlong)((*piVar9 / 100) * (((iVar10 >> 6) - (iVar10 >> 0x1f)) + iVar8 / 1000)) *
                0x68db8bad;
        _DAT_1fffad58 = (undefined2)((uint)(iVar7 * 100) >> 9);
        _DAT_1fffad5a = (short)(uint)((ulonglong)lVar2 >> 0x2c) - (short)(lVar2 >> 0x3f);
        FUN_00020c0e(&DAT_1fffb064,iVar7);
        FUN_00020c0e(&DAT_1fffb044,(int)(iVar8 + ((uint)(iVar8 >> 0x1f) >> 0x1e)) >> 2);
        FUN_00020c48(&DAT_1fffb064,_DAT_1fffb064);
        FUN_00020c48(&DAT_1fffb044,_DAT_1fffb044);
        uVar16 = (uint)DAT_1fffb080;
        if (uVar16 == DAT_1fffb081) {
          iVar7 = (_DAT_1fffb074 - (_DAT_1fffb074 / (int)(uVar16 * 0x10)) * uVar16 * 0x10) * 0xc35;
          FUN_00017b1c(((int)(_DAT_1fffb074 / (int)uVar16 +
                             ((uint)(_DAT_1fffb074 / (int)uVar16 >> 0x1f) >> 0x1c)) >> 4) * 0xc35 +
                       ((int)(iVar7 + ((uint)(iVar7 >> 0x1f) >> 0x1c)) >> 4) / (int)uVar16,
                       _DAT_1fffb054 / (int)(uint)DAT_1fffb060 << 2);
          _DAT_1fffad08 = FUN_0000701e();
          _DAT_1fffb074 = 0;
          DAT_1fffb080 = 0;
          _DAT_1fffb070 = 0;
          _DAT_1fffb078 = 0x80000000;
          _DAT_1fffb07c = 0x7fffffff;
          _DAT_1fffb054 = 0;
          DAT_1fffb060 = 0;
          _DAT_1fffb050 = 0;
          _DAT_1fffb058 = 0x80000000;
          _DAT_1fffb05c = 0x7fffffff;
        }
      }
    } while (sRam40040004 == 0);
    do {
    } while ((bRam40040046 & 1) == 0);
    bRam40040046 = bRam40040046 & 0xfe;
    if (500 < uRam40040050) {
      cStack_54 = cStack_54 + '\x01';
    }
    if (500 < uRam40040052) {
      unaff_r8 = unaff_r8 + '\x01';
    }
    uVar16 = (uint)_DAT_40040070;
    if (!bVar20) {
      puVar11 = (undefined2 *)&DAT_1fffad1e;
      puVar6 = auStack_48;
      uVar14 = uVar18;
      do {
        puVar1 = puVar6 + -2;
        uVar14 = uVar14 - 1;
        *puVar6 = (uint)*(ushort *)*puVar1 + *puVar6;
        puVar6 = puVar6 + 3;
        uVar5 = (undefined2)((0x438000 / uVar16) * (uint)*(ushort *)*puVar1 >> 0xc);
        puVar11[5] = uVar5;
        *puVar11 = uVar5;
        puVar11 = puVar11 + 1;
      } while (uVar14 != 0);
    }
    iVar7 = FUN_00021cb4();
    iVar8 = FUN_00021c94();
    if ((_DAT_42a700c8 != 0) && (-1 < iVar7)) {
      _DAT_1fffad2c = (short)iVar7 * 100 + (*puStack_38 & 0xf);
    }
    if ((_DAT_42a700cc != 0) && (-1 < iVar8)) {
      _DAT_1fffad2e = (short)iVar8 * 100 + (*puStack_2c & 0xf);
    }
    _DAT_1fffad3e = _DAT_1fffad3e | 8;
    iVar19 = iVar19 + 1;
    _DAT_1fffad30 = (undefined2)(0x438000 / uVar16);
    if (bVar20) {
      if (((uint)(iVar19 * -0x3d70a3d7) >> 1 | iVar19 * -0x80000000) < 0x51eb852) {
LAB_0001e2ba:
        _DAT_1fffad3e = _DAT_1fffad3e | 0x30;
      }
    }
    else {
      uVar16 = 0;
      do {
        aiStack_14[uVar16] = (uint)*(ushort *)(&DAT_1fffad28 + uVar16 * 2) + aiStack_14[uVar16];
        uVar16 = uVar16 + 1;
      } while (uVar18 != uVar16);
      if (((uint)(iVar19 * -0x3d70a3d7) >> 1 | iVar19 * -0x80000000) < 0x51eb852) {
        psVar12 = (short *)&DAT_1fffad14;
        piVar9 = aiStack_14;
        uVar16 = uVar18;
        do {
          uVar16 = uVar16 - 1;
          *psVar12 = (short)(uint)((ulonglong)((longlong)*piVar9 * 0x51eb851f) >> 0x24) -
                     (short)((longlong)*piVar9 * 0x51eb851f >> 0x3f);
          *piVar9 = 0;
          psVar12 = psVar12 + 1;
          piVar9 = piVar9 + 1;
        } while (uVar16 != 0);
        goto LAB_0001e2ba;
      }
    }
    if (iVar19 == 0xfa) {
      uVar3 = 0x41eb0000 / ((ulonglong)uStack_18 / 10);
      if (bVar4) {
        puVar11 = (undefined2 *)&DAT_1fffad0a;
        puVar6 = auStack_48;
        iVar19 = uVar18 - 1;
        do {
          lVar2 = *puVar6 * uVar3;
          uVar5 = FUN_00004474((int)lVar2,(int)((ulonglong)lVar2 >> 0x20),0xfa000,0);
          *puVar11 = uVar5;
          iVar19 = iVar19 + -1;
          *puVar6 = 0;
          puVar11 = puVar11 + 1;
          puVar6 = puVar6 + 3;
        } while (iVar19 != 0);
      }
      if ((_DAT_42a700c8 != 0) && (-1 < iVar7)) {
        _DAT_1fffad0e = _DAT_1fffad2c * 10;
      }
      if ((_DAT_42a700cc != 0) && (-1 < iVar8)) {
        _DAT_1fffad10 = _DAT_1fffad2e * 10;
      }
      iVar19 = 0;
      _DAT_1fffad3e = _DAT_1fffad3e | 0x40;
      _DAT_1fffad12 = (undefined2)uVar3;
      uStack_18 = 0;
      DAT_1fffad55 = cStack_54;
      DAT_1fffad56 = unaff_r8;
      cStack_54 = '\0';
      unaff_r8 = '\0';
    }
  } while( true );
}

