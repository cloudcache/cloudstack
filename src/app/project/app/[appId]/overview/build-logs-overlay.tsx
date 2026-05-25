import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import React, { useEffect } from "react";
import { set } from "date-fns";
import { DeploymentInfoModel } from "@/shared/model/deployment-info.model";
import LogsStreamed from "../../../../../components/custom/logs-streamed";
import { formatDateTime } from "@/frontend/utils/format.utils";
import BuildLogsStreamed from "@/components/custom/build-logs-streamed";

export function BuildLogsDialog({
  deploymentInfo,
  onClose
}: {
  deploymentInfo?: DeploymentInfoModel;
  onClose: () => void;
}) {

  if (!deploymentInfo) {
    return <></>;
  }

  return (
    <Dialog open={!!deploymentInfo} onOpenChange={(isO) => {
      onClose();
    }}>
      <DialogContent className="sm:max-w-[1300px]">
        <DialogHeader>
          <DialogTitle>Deployment Logs</DialogTitle>
          <DialogDescription>
            View the logs for the selected deployment {formatDateTime(deploymentInfo.createdAt)}.
          </DialogDescription>
        </DialogHeader>
        <div >
          {!deploymentInfo.deploymentId && 'For this build is no log available'}
          {deploymentInfo.deploymentId && <BuildLogsStreamed deploymentId={deploymentInfo.deploymentId} />}
        </div>
      </DialogContent>
    </Dialog>
  )
}
