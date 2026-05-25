'use client'

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Toast } from "@/frontend/utils/toast.utils";
import { useConfirmDialog } from "@/frontend/states/zustand.states";
import { setNodeSchedulable } from "./actions";
import { useState } from "react";

interface NodeItem {
    id: string;
    hostname: string;
    ip_address: string;
    node_role?: string;
    node_status: string;
    cpu_capacity_mcores?: number;
    mem_capacity_mb?: number;
    storage_path?: string;
    cluster_name?: string;
    pool_name?: string;
    last_seen_at?: string;
}

export default function AdminNodesTab({ initialItems }: { initialItems: NodeItem[] }) {
    const [items, setItems] = useState<NodeItem[]>(initialItems);
    const { openConfirmDialog } = useConfirmDialog();

    const handleCordon = async (node: NodeItem, schedulable: boolean) => {
        const confirmed = await openConfirmDialog({
            title: schedulable ? 'Activate Node' : 'Cordon Node',
            description: schedulable
                ? `Mark node ${node.hostname} as schedulable so workloads can be placed on it.`
                : `Cordon node ${node.hostname}. No new workloads will be scheduled on this node.`,
            okButton: schedulable ? 'Activate' : 'Cordon',
            cancelButton: 'Cancel',
        });
        if (!confirmed) return;
        Toast.fromAction(() => setNodeSchedulable(node.id, schedulable));
    };

    const isActive = (node: NodeItem) => node.node_status === 'ACTIVE' || node.node_status === 'ready';

    return (
        <Card>
            <CardHeader>
                <CardTitle>Nodes</CardTitle>
                <CardDescription>All registered nodes across clusters. Use cordon/uncordon to manage scheduling.</CardDescription>
            </CardHeader>
            <CardContent>
                {items.length === 0 ? (
                    <p className="text-muted-foreground text-sm">No nodes registered yet.</p>
                ) : (
                    <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
                        {items.map((node) => (
                            <div key={node.id} className="border rounded-lg p-4 space-y-2">
                                <div className="flex items-center justify-between">
                                    <span className="font-semibold truncate">{node.hostname}</span>
                                    <Badge variant={isActive(node) ? 'default' : 'destructive'}>
                                        {node.node_status}
                                    </Badge>
                                </div>
                                <div className="text-sm text-muted-foreground space-y-1">
                                    <div><span className="font-medium">IP:</span> {node.ip_address}</div>
                                    {node.node_role && <div><span className="font-medium">Role:</span> {node.node_role}</div>}
                                    {node.cluster_name && <div><span className="font-medium">Cluster:</span> {node.cluster_name}</div>}
                                    {node.pool_name && <div><span className="font-medium">Pool:</span> {node.pool_name}</div>}
                                    {node.cpu_capacity_mcores !== undefined && (
                                        <div><span className="font-medium">CPU:</span> {(node.cpu_capacity_mcores / 1000).toFixed(1)} cores</div>
                                    )}
                                    {node.mem_capacity_mb !== undefined && (
                                        <div><span className="font-medium">Memory:</span> {(node.mem_capacity_mb / 1024).toFixed(1)} GB</div>
                                    )}
                                    {node.storage_path && <div><span className="font-medium">Storage:</span> {node.storage_path}</div>}
                                    {node.last_seen_at && (
                                        <div><span className="font-medium">Last seen:</span> {new Date(node.last_seen_at).toLocaleString()}</div>
                                    )}
                                </div>
                                <div className="flex gap-2 pt-2">
                                    {isActive(node) ? (
                                        <Button size="sm" variant="outline" onClick={() => handleCordon(node, false)}>
                                            Cordon
                                        </Button>
                                    ) : (
                                        <Button size="sm" variant="outline" onClick={() => handleCordon(node, true)}>
                                            Uncordon
                                        </Button>
                                    )}
                                </div>
                            </div>
                        ))}
                    </div>
                )}
            </CardContent>
        </Card>
    );
}
