// The board's card for one Dark Elf finding.

export interface Finding {
  claim: string;
  file: string;
}

export function showFinding(root: HTMLElement, finding: Finding): void {
  const card = document.createElement('div');
  card.className = 'finding';
  card.innerHTML = `<h3>${finding.file}</h3><p>${finding.claim}</p>`;
  root.appendChild(card);
}
