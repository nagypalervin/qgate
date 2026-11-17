export class BComponent {
  ngOnInit(): string {
    switch (this.mode) {
      case 0: return 'mode 0';
      case 1: return 'mode 1';
      case 2: return 'mode 2';
      case 3: return 'mode 3';
      case 4: return 'mode 4';
      case 5: return 'mode 5';
      case 6: return 'mode 6';
      case 7: return 'mode 7';
      case 8: return 'mode 8';
      case 9: return 'mode 9';
      case 10: return 'mode 10';
      case 11: return 'mode 11';
      case 12: return 'mode 12';
    }
    return 'none';
  }
}
