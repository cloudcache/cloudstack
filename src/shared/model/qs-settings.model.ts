import { stringToBoolean } from "@/shared/utils/zod.utils";
import { z } from "zod";

export const qsIngressSettingsZodModel = z.object({
  serverUrl: z.string().trim().min(1),
  disableNodePortAccess: stringToBoolean,
})

export type QsIngressSettingsModel = z.infer<typeof qsIngressSettingsZodModel>;