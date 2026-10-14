import en from "./i18n/en.json";

export const MESSAGES: Record<string, string> = en;

/** Looks up a UI text; `{name}` placeholders are replaced from `params`. */
export function t(key: string, params: Record<string, string | number> = {}): string {
  const text = MESSAGES[key];
  if (text === undefined) {
    console.warn(`missing i18n key: ${key}`);
    return key;
  }
  return text.replace(/\{(\w+)\}/g, (_, name: string) => String(params[name] ?? `{${name}}`));
}
