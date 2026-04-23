import { BicepsFlexed, Cpu, Gpu } from 'lucide-react';
import type { SystemResources } from '../types/apps';
import { formatBytes } from '../utils/formatters';
import { Tooltip } from './ui';

interface HeaderResourceStripProps {
  appResources?: {
    gpu_memory?: number;
    ram_memory?: number;
  };
  systemResources?: SystemResources;
}

interface HeaderResourceMetrics {
  cpuUsage: number;
  gpuUsage: number;
  ramPercent: number;
  ramUsed: number;
  vramPercent: number;
  vramUsed: number;
}

interface RamMetrics {
  ramPercent: number;
  ramUsed: number;
}

interface VramMetrics {
  vramPercent: number;
  vramUsed: number;
}

function getLoadColor(load: number): string {
  if (load < 30) {
    return 'hsl(var(--accent-info))';
  }
  if (load < 70) {
    return 'hsl(var(--accent-warning))';
  }
  return 'hsl(var(--accent-error))';
}

function getRamMetrics(systemResources?: SystemResources): RamMetrics {
  const ramPercent = Math.round(systemResources?.ram.usage ?? 0);
  const ramTotal = systemResources?.ram.total ?? 0;
  return {
    ramPercent,
    ramUsed: (ramTotal * ramPercent) / 100,
  };
}

function getVramMetrics({
  appResources,
  systemResources,
}: HeaderResourceStripProps): VramMetrics {
  const vramTotal = systemResources?.gpu.memory_total ?? 0;
  const vramUsedSystem = systemResources?.gpu.memory ?? 0;
  const vramUsed = Math.max(vramUsedSystem, appResources?.gpu_memory ?? 0);

  return {
    vramPercent: vramTotal > 0 ? Math.min(100, Math.round((vramUsed / vramTotal) * 100)) : 0,
    vramUsed,
  };
}

function getHeaderResourceMetrics(props: HeaderResourceStripProps): HeaderResourceMetrics {
  const ram = getRamMetrics(props.systemResources);
  const vram = getVramMetrics(props);

  return {
    cpuUsage: Math.round(props.systemResources?.cpu.usage ?? 0),
    gpuUsage: Math.round(props.systemResources?.gpu.usage ?? 0),
    ramPercent: ram.ramPercent,
    ramUsed: ram.ramUsed,
    vramPercent: vram.vramPercent,
    vramUsed: vram.vramUsed,
  };
}

export function HeaderResourceStrip({
  appResources,
  systemResources,
}: HeaderResourceStripProps) {
  const metrics = getHeaderResourceMetrics({ appResources, systemResources });

  return (
    <div className="h-3 px-3 pb-1.5 flex items-center gap-2">
      <div className="flex items-center gap-1 flex-shrink-0">
        <Tooltip content={`${metrics.cpuUsage}%`}>
          <BicepsFlexed
            className="w-3.5 h-3.5"
            style={{ color: getLoadColor(metrics.cpuUsage) }}
          />
        </Tooltip>
        <Tooltip content={`RAM ${formatBytes(metrics.ramUsed * 1024 * 1024 * 1024)}`}>
          <Cpu className="w-3.5 h-3.5 text-[hsl(var(--launcher-accent-cpu))]" />
        </Tooltip>
      </div>

      <div className="flex-1 flex items-center gap-0.5 min-w-0">
        <div className="flex-1 h-1.5 bg-[hsl(var(--surface-overlay))] rounded-sm overflow-hidden">
          <div
            className="h-full bg-[hsl(var(--launcher-accent-ram))] transition-all duration-300"
            style={{ width: `${metrics.ramPercent}%` }}
          />
        </div>

        <div className="flex-1 h-1.5 bg-[hsl(var(--surface-overlay))] rounded-sm overflow-hidden flex justify-end">
          <div
            className="h-full bg-[hsl(var(--launcher-accent-gpu))] transition-all duration-300"
            style={{ width: `${metrics.vramPercent}%` }}
          />
        </div>
      </div>

      <div className="flex items-center gap-1 flex-shrink-0">
        <Tooltip content={`VRAM ${formatBytes(metrics.vramUsed * 1024 * 1024 * 1024)}`}>
          <Gpu className="w-3.5 h-3.5 text-[hsl(var(--launcher-accent-gpu))]" />
        </Tooltip>
        <Tooltip content={`${metrics.gpuUsage}%`}>
          <BicepsFlexed
            className="w-3.5 h-3.5 scale-x-[-1]"
            style={{ color: getLoadColor(metrics.gpuUsage) }}
          />
        </Tooltip>
      </div>
    </div>
  );
}
