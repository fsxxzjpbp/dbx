import type { DatabaseType } from "@/types/database";
import { isSchemaAware, usesTreeSchemaMode } from "./databaseFeatureSupport";

export type SyntheticEditKey = "oracle-rowid" | "neo4j-element-id";

export interface TableDataCapability {
  insert: boolean;
  updateRequiresPrimaryKey: boolean;
  deleteRequiresPrimaryKey: boolean;
  requiresTransactionalTableForExistingRows: boolean;
  transaction: boolean;
  readonly?: boolean;
}

export interface DatabaseCapability {
  schemaAware: boolean;
  treeSchemaMode: boolean;
  tableData: TableDataCapability;
  syntheticKey?: SyntheticEditKey;
}

const DEFAULT_TABLE_DATA_CAPABILITY: TableDataCapability = {
  insert: false,
  updateRequiresPrimaryKey: true,
  deleteRequiresPrimaryKey: true,
  requiresTransactionalTableForExistingRows: false,
  transaction: true,
};

const DEFAULT_CAPABILITY: DatabaseCapability = {
  schemaAware: false,
  treeSchemaMode: false,
  tableData: DEFAULT_TABLE_DATA_CAPABILITY,
};

const DATABASE_CAPABILITY_OVERRIDES: Partial<Record<DatabaseType, Partial<DatabaseCapability>>> = {
  hive: {
    tableData: {
      insert: true,
      updateRequiresPrimaryKey: false,
      deleteRequiresPrimaryKey: false,
      requiresTransactionalTableForExistingRows: true,
      transaction: false,
    },
  },
  informix: {
    tableData: {
      insert: true,
      updateRequiresPrimaryKey: true,
      deleteRequiresPrimaryKey: true,
      requiresTransactionalTableForExistingRows: false,
      transaction: true,
    },
  },
  neo4j: {
    syntheticKey: "neo4j-element-id",
  },
  oracle: {
    syntheticKey: "oracle-rowid",
  },
  trino: {
    tableData: {
      insert: true,
      updateRequiresPrimaryKey: true,
      deleteRequiresPrimaryKey: true,
      requiresTransactionalTableForExistingRows: false,
      transaction: false,
    },
  },
  clickhouse: {
    tableData: {
      insert: false,
      updateRequiresPrimaryKey: true,
      deleteRequiresPrimaryKey: true,
      requiresTransactionalTableForExistingRows: false,
      transaction: false,
      readonly: true,
    },
  },
};

export function getDatabaseCapability(dbType?: DatabaseType): DatabaseCapability {
  const override = dbType ? DATABASE_CAPABILITY_OVERRIDES[dbType] : undefined;
  return {
    ...DEFAULT_CAPABILITY,
    ...override,
    schemaAware: isSchemaAware(dbType),
    treeSchemaMode: usesTreeSchemaMode(dbType),
    tableData: {
      ...DEFAULT_TABLE_DATA_CAPABILITY,
      ...override?.tableData,
    },
  };
}
