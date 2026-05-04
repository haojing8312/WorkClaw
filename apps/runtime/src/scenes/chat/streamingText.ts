export function mergeStreamingTextChunk(currentText: string, incomingText: string): string {
  if (!incomingText) return currentText;
  if (!currentText) return incomingText;
  if (currentText.endsWith(incomingText)) return currentText;
  if (incomingText.startsWith(currentText)) return incomingText;

  const maxOverlap = Math.min(currentText.length, incomingText.length);
  for (let overlap = maxOverlap; overlap > 0; overlap -= 1) {
    if (currentText.slice(-overlap) === incomingText.slice(0, overlap)) {
      return currentText + incomingText.slice(overlap);
    }
  }

  return currentText + incomingText;
}
