using System;
using System.Threading.Tasks;
using uniffi.pumas_uniffi;

internal static class Program
{
    public static async Task<int> Main()
    {
        string version = PumasUniffiMethods.Version();
        if (string.IsNullOrWhiteSpace(version))
        {
            throw new InvalidOperationException("PumasUniffiMethods.Version() returned an empty value.");
        }

        var config = new FfiApiConfig(
            launcherRoot: "/tmp/pumas-csharp-smoke",
            autoCreateDirs: true,
            enableHf: false
        );

        var request = new FfiDownloadRequest(
            repoId: "repo/model",
            family: "llm",
            officialName: "Model",
            modelType: "llm",
            quant: null,
            filename: null,
            filenames: null,
            pipelineTag: null
        );

        if (!config.autoCreateDirs || request.family != "llm")
        {
            throw new InvalidOperationException("Generated record construction produced unexpected values.");
        }

        Console.WriteLine($"Pumas version: {version}");
        Console.WriteLine($"Config launcher root: {config.launcherRoot}");
        Console.WriteLine($"Download request family: {request.family}");
        await Task.CompletedTask;
        return 0;
    }
}
