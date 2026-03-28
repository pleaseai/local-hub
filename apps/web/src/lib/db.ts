import path from "node:path";
import { PrismaLibSql } from "@prisma/adapter-libsql";
import { PrismaClient } from "../generated/prisma/client";

const globalForPrisma = globalThis as unknown as { prisma: InstanceType<typeof PrismaClient> | undefined };

const dbUrl = process.env.DATABASE_URL ?? `file:${path.resolve("prisma/dev.db")}`;

function makePrisma() {
	const adapter = new PrismaLibSql({ url: dbUrl });
	return new PrismaClient({ adapter });
}

export const prisma = globalForPrisma.prisma ?? (globalForPrisma.prisma = makePrisma());
