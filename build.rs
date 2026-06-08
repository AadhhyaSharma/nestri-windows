/// build.rs — Windows build configuration
/// - Embeds a Windows app manifest (DPI awareness, UTF-8 locale)
/// - Sets linker subsystem to Windows (no console window)

fn main() {
    // Only apply Windows-specific build steps on Windows
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();

        // Embed app manifest for DPI awareness + proper UTF-8 handling
        res.set_manifest(r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity
    version="1.0.0.0"
    processorArchitecture="amd64"
    name="Nestri.Server"
    type="win32"
  />
  <description>Nestri Windows Streaming Server</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <!-- Windows 10/11 -->
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
  </application>
</assembly>
        "#);

        res.compile().expect("Failed to compile Windows resources");
    }

    // Print cargo rerun conditions
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/");
}
