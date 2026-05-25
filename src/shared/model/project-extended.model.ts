import { App, Project } from "@/shared/model/prisma-compat";

export type ProjectExtendedModel = Project & {
    apps: App[];
}