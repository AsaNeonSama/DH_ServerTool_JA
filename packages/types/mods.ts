export type ModGroup = "default" | "single" | "bone_dagger";

export type ModKind = "single" | "mode";

export interface PatchScript {
  readonly id: string;
  readonly fileName: string;
  readonly label: string;
  readonly group: ModGroup;
  readonly source: string;
}

export interface ModMenuOption {
  readonly number: number;
  readonly label: string;
  readonly kind: ModKind;
  readonly scripts: readonly PatchScript[];
}
