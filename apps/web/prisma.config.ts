import path from "node:path";
import "dotenv/config";
import { defineConfig } from "prisma/config";

export default defineConfig({
	schema: "prisma/schema.prisma",
	migrations: {
		path: "prisma/migrations",
	},
	datasource: {
		url:
			process.env.DATABASE_URL ??
			`file:${path.resolve("prisma/dev.db")}`,
	},
});
