import React from 'react';
import { Cpu, Gpu, HardDrive, MemoryStick } from 'lucide-react';
import type { SystemResources } from '../types/apps';

interface ResourceMonitorProps {
  resources?: SystemResources;
}

const ResourceStat: React.FC<{
  label: string;
  value: string | number;
  icon: React.ReactNode;
  accentColor: string;
  memoryValue?: string | number;
}> = ({ value, icon, accentColor, memoryValue }) => {
  return (
    <div className={`flex items-center gap-4 px-3 py-1 bg-[hsl(var(--launcher-accent-${accentColor})/0.1)] rounded border border-[hsl(var(--launcher-accent-${accentColor})/0.3)]`}>
      <div className="flex items-center gap-1">
        {icon}
        <span className="font-medium text-[hsl(var(--launcher-text-primary))] font-mono text-xs">
          {value}
        </span>
      </div>
      {memoryValue !== undefined && (
        <div className="flex items-baseline gap-1">
          <MemoryStick className="w-3 h-3 text-[hsl(var(--launcher-text-secondary))]" />
          <span className="font-medium text-[hsl(var(--launcher-text-primary))] font-mono text-xs">
            {memoryValue}
          </span>
        </div>
      )}
    </div>
  );
};

export const ResourceMonitor: React.FC<ResourceMonitorProps> = ({ resources }) => {
  // Default values if resources are not provided
  const gpuUsage = Math.round(resources?.gpu.usage ?? 0);
  const gpuMemory = resources?.gpu.memory ?? 0;
  const cpuUsage = Math.round(resources?.cpu.usage ?? 0);
  const ramUsage = resources?.ram.usage ?? 0;

  return (
    <div className="flex flex-col gap-1">
      <ResourceStat
        label="GPU"
        value={gpuUsage}
        memoryValue={gpuMemory.toFixed(1)}
        icon={<Gpu className="w-3 h-3 text-[hsl(var(--launcher-accent-gpu))]" />}
        accentColor="gpu"
      />
      <ResourceStat
        label="CPU"
        value={cpuUsage}
        memoryValue={ramUsage.toFixed(1)}
        icon={<Cpu className="w-3 h-3 text-[hsl(var(--launcher-accent-cpu))]" />}
        accentColor="cpu"
      />
    </div>
  );
};

export const DiskMonitor: React.FC<{ diskFree?: number }> = ({ diskFree = 0 }) => {
  return (
    <div className="px-4 py-2">
      <div className="flex items-center gap-1">
        <HardDrive className="w-3 h-3 text-[hsl(var(--launcher-text-secondary))]" />
        <span className="font-medium text-[hsl(var(--launcher-text-primary))] font-mono text-xs">
          {diskFree}
        </span>
      </div>
    </div>
  );
};
