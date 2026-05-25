import { stringToBoolean, stringToNumber } from "@/shared/utils/zod.utils";
import { z } from "zod";

export const appDomainEditZodModel = z.object({
  hostname: z.string().trim().min(1),
  useSsl: stringToBoolean,
  redirectHttps: stringToBoolean,
  port: stringToNumber,
})

export type AppDomainEditModel = z.infer<typeof appDomainEditZodModel>;