import { strict as assert } from "node:assert";
import test from "node:test";
import {
  DBX_NEO4J_ELEMENT_ID_COLUMN,
  DBX_ROWID_COLUMN,
  canEditExistingTableRows,
  editablePrimaryKeys,
  hiveTablePropertiesIndicateTransactional,
  isHiddenGridColumn,
  isTableDataEditable,
  supportsDataGridTransaction,
  usesSyntheticRowIdKey,
} from "../src/lib/tableEditing.ts";
import type { ColumnInfo } from "../src/types/database.ts";

function column(name: string, isPrimaryKey = false): ColumnInfo {
  return {
    name,
    data_type: "VARCHAR2",
    is_nullable: true,
    column_default: null,
    is_primary_key: isPrimaryKey,
    extra: null,
  };
}

test("uses ROWID as Oracle editable key when a table has no primary key", () => {
  assert.deepEqual(editablePrimaryKeys("oracle", [column("ID"), column("CITY")]), [DBX_ROWID_COLUMN]);
});

test("keeps declared primary keys ahead of Oracle ROWID fallback", () => {
  assert.deepEqual(editablePrimaryKeys("oracle", [column("ID", true), column("CITY")]), ["ID"]);
});

test("does not synthesize ROWID for non-Oracle keyless tables", () => {
  assert.deepEqual(editablePrimaryKeys("mysql", [column("ID"), column("CITY")]), []);
});

test("allows Hive table data editing even without declared primary keys", () => {
  assert.equal(isTableDataEditable("hive", []), true);
  assert.equal(isTableDataEditable("mysql", []), false);
  assert.equal(isTableDataEditable("postgres", ["id"]), true);
});

test("does not use transactional grid saves for Hive", () => {
  assert.equal(supportsDataGridTransaction("hive"), false);
  assert.equal(supportsDataGridTransaction("postgres"), true);
});

test("allows existing row edits for Hive only when the table is transactional", () => {
  assert.equal(canEditExistingTableRows("hive", true), true);
  assert.equal(canEditExistingTableRows("hive", false), false);
  assert.equal(canEditExistingTableRows("hive", undefined), false);
  assert.equal(canEditExistingTableRows("postgres", undefined), true);
});

test("detects transactional Hive table properties", () => {
  assert.equal(
    hiveTablePropertiesIndicateTransactional({
      columns: ["prpt_name", "prpt_value"],
      rows: [["transactional", "true"]],
    }),
    true,
  );
  assert.equal(
    hiveTablePropertiesIndicateTransactional({
      columns: ["prpt_name", "prpt_value"],
      rows: [["Table testdb.departments does not have property: transactional", null]],
    }),
    false,
  );
});

test("uses elementId as Neo4j editable key when labels have no primary key", () => {
  assert.deepEqual(editablePrimaryKeys("neo4j", [column("name"), column("role")]), [DBX_NEO4J_ELEMENT_ID_COLUMN]);
});

test("detects the synthetic Oracle ROWID key case", () => {
  assert.equal(usesSyntheticRowIdKey("oracle", [DBX_ROWID_COLUMN]), true);
  assert.equal(usesSyntheticRowIdKey("oracle", [DBX_ROWID_COLUMN.toLowerCase()]), true);
  assert.equal(usesSyntheticRowIdKey("postgres", [DBX_ROWID_COLUMN]), false);
  assert.equal(usesSyntheticRowIdKey("oracle", ["ID"]), false);
  assert.equal(usesSyntheticRowIdKey("neo4j", [DBX_NEO4J_ELEMENT_ID_COLUMN]), true);
});

test("hides only the synthetic Oracle ROWID grid column", () => {
  assert.equal(isHiddenGridColumn("oracle", DBX_ROWID_COLUMN, [DBX_ROWID_COLUMN]), true);
  assert.equal(isHiddenGridColumn("oracle", "ROWID", [DBX_ROWID_COLUMN]), false);
  assert.equal(isHiddenGridColumn("mysql", DBX_ROWID_COLUMN, [DBX_ROWID_COLUMN]), false);
  assert.equal(isHiddenGridColumn("neo4j", DBX_NEO4J_ELEMENT_ID_COLUMN, [DBX_NEO4J_ELEMENT_ID_COLUMN]), true);
});
