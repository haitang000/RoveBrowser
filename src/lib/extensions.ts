export interface ExtensionItem {
  id: string;
  name: string;
  version: string;
  enabled: boolean;
}

export interface ExtensionReview {
  ticket: string;
  name: string;
  version: string;
  permissions: string[];
  hosts: string[];
}

export interface ExtensionsState {
  items: ExtensionItem[] | null;
  review: ExtensionReview | null;
  busy: boolean;
  error: string | null;
}
