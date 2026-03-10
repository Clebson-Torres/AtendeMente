export function stringToHealthHistoryItems(value: string) {
  return value
    .split("\n")
    .map((item) => item.trim())
    .filter(Boolean);
}

export function healthHistoryItemsToString(items: string[]) {
  return items.join("\n");
}

export function buildHealthHistoryValue(items: string[], draftValue: string) {
  const nextDraft = draftValue.trim();

  if (!nextDraft || items.includes(nextDraft)) {
    return healthHistoryItemsToString(items);
  }

  return healthHistoryItemsToString([...items, nextDraft]);
}
