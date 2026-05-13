import type { ColumnInfo, DatabaseType } from "@/types/database";

export const DBX_ROWID_COLUMN = "__DBX_ROWID";
export const DBX_NEO4J_ELEMENT_ID_COLUMN = "__DBX_ELEMENT_ID";

export function editablePrimaryKeys(databaseType: DatabaseType | undefined, columns: ColumnInfo[]): string[] {
  const primaryKeys = columns.filter((column) => column.is_primary_key).map((column) => column.name);
  if (databaseType === "oracle" && primaryKeys.length === 0) return [DBX_ROWID_COLUMN];
  if (databaseType === "neo4j" && primaryKeys.length === 0) return [DBX_NEO4J_ELEMENT_ID_COLUMN];
  return primaryKeys;
}

export function isTableDataEditable(databaseType: DatabaseType | undefined, primaryKeys: string[]): boolean {
  if (databaseType === "hive") return true;
  return primaryKeys.length > 0;
}

export function supportsDataGridTransaction(databaseType: DatabaseType | undefined): boolean {
  return databaseType !== "hive";
}

export function canEditExistingTableRows(
  databaseType: DatabaseType | undefined,
  hiveTableTransactional?: boolean,
): boolean {
  if (databaseType === "hive") return hiveTableTransactional === true;
  return true;
}

export function hiveTablePropertiesIndicateTransactional(result: { rows: readonly (readonly unknown[])[] }): boolean {
  return result.rows.some((row) => {
    const name = String(row[0] ?? "")
      .trim()
      .toLowerCase();
    const value = String(row[1] ?? "")
      .trim()
      .toLowerCase();
    return name === "transactional" && value === "true";
  });
}

export function usesSyntheticRowIdKey(databaseType: DatabaseType | undefined, primaryKeys: string[]): boolean {
  return (
    primaryKeys.length === 1 &&
    ((databaseType === "oracle" && primaryKeys[0].toUpperCase() === DBX_ROWID_COLUMN) ||
      (databaseType === "neo4j" && primaryKeys[0] === DBX_NEO4J_ELEMENT_ID_COLUMN))
  );
}

export function isHiddenGridColumn(
  databaseType: DatabaseType | undefined,
  column: string,
  primaryKeys: string[],
): boolean {
  if (databaseType === "neo4j" && column === DBX_NEO4J_ELEMENT_ID_COLUMN) return true;
  return usesSyntheticRowIdKey(databaseType, primaryKeys) && column.toUpperCase() === DBX_ROWID_COLUMN;
}
