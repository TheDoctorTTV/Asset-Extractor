document.addEventListener("DOMContentLoaded", () => {
    // Initialize with the Converter tab
    showConverter();
  });
  
  // Utility to clear the content area
  function clearContent() {
    const content = document.getElementById("content");
    content.innerHTML = "";
  }
  
  // Converter tab logic
  function showConverter() {
    clearContent();
  
    const content = document.getElementById("content");
    content.innerHTML = `
      <h2>GZ.Asset to .DAT Converter</h2>
      <div class="input-group">
        <label for="input-folder">Input Folder:</label>
        <input type="file" id="input-folder" webkitdirectory directory multiple />
      </div>
      <div class="input-group">
        <label for="output-folder">Output Folder:</label>
        <input type="file" id="output-folder" webkitdirectory directory />
      </div>
      <button onclick="startConversion()">Start Conversion</button>
    `;
  }
  
  // Extractor tab logic
  function showExtractor() {
    clearContent();
  
    const content = document.getElementById("content");
    content.innerHTML = `
      <h2>Unity Asset Bundle Extractor</h2>
      <div class="input-group">
        <label for="asset-file">Select Unity Asset Bundle File:</label>
        <input type="file" id="asset-file" />
      </div>
      <button onclick="extractAssetBundle()">Extract</button>
    `;
  }
  
  // Conversion logic
  function startConversion() {
    const inputFolder = document.getElementById("input-folder").files;
    const outputFolder = document.getElementById("output-folder").files;
  
    if (!inputFolder.length || !outputFolder.length) {
      alert("Please select both input and output folders.");
      return;
    }
  
    Array.from(inputFolder).forEach(file => {
      if (file.name.endsWith(".gz.asset")) {
        const reader = new FileReader();
        reader.onload = function (event) {
          try {
            const decompressed = pako.inflate(event.target.result);
            const outputFileName = file.name.replace(".gz.asset", ".dat");
            saveFile(new Blob([decompressed]), outputFileName);
            console.log(`Converted: ${file.name} -> ${outputFileName}`);
          } catch (err) {
            console.error(`Error processing ${file.name}: ${err.message}`);
          }
        };
        reader.readAsArrayBuffer(file);
      }
    });
  
    alert("Conversion complete!");
  }
  
  // Extraction logic
  function extractAssetBundle() {
    const assetFile = document.getElementById("asset-file").files[0];
    if (!assetFile) {
      alert("Please select a Unity Asset Bundle file.");
      return;
    }
  
    // Placeholder: Asset bundle extraction requires Unity-specific tools
    alert("Asset bundle extraction is not yet implemented.");
  }
  
  // Utility to save files
  function saveFile(blob, fileName) {
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = fileName;
    a.click();
  }
  