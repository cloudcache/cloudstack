import { User, UserGroup } from "@/shared/model/prisma-compat";
import { UserGroupExtended } from "./sim-session.model";

export type UserExtended = {
    id: string;
    username: string;
    userGroup: UserGroup | null;
    userGroupId: string | null;
    email: string;
    createdAt: Date;
    updatedAt: Date;
};
