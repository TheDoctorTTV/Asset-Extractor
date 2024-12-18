# **Beat Saber Mapping Extractor**

This tool allows you to extract and process Unity asset files (such as `.gz.asset`) for Beat Saber maps. It simplifies the process of converting and exporting assets for use or further modification.

> ⚠️ **Disclaimer**: This tool is provided for **educational purposes only**. Use it responsibly and respect the rights of content creators.

---

## **Features**
- Extract Unity asset bundles with a user-friendly GUI.
- Convert `.gz.asset` files to `.dat` files easily.
- Automates the process of preparing assets for use in tools like **AssetStudio**.

---

## **How to Use the Tool**

### **Step 1: Run the Program**
1. Run the `BeatSaberMappingExtractor.exe` file.
2. The GUI will open, and you will see two main options:
   - **GZ.Asset to .DAT Converter**.
   - **Unity Asset Extractor**.

---

### **Step 2: Extract Unity Assets**
1. Select the **"Unity Asset Extractor"** tab.
2. Press **"Browse File"** and locate your Unity asset file (e.g., `.unity3d`).
3. The tool will:
   - Extract the raw asset bundle into an `extracted_assets` folder.
   - Save the `raw_asset_bundle.unity3d` file for further use.

---

### **Step 3: Use AssetStudio to Export Assets**
1. Download **[AssetStudio](https://github.com/Perfare/AssetStudio)**.
2. Open `AssetStudioGUI.exe`.
3. Drag the `raw_asset_bundle.unity3d` file (from the `extracted_assets` folder) into AssetStudio.
4. In AssetStudio:
   - Click **"Export"** at the top.
   - Select **"All Assets"**.
   - Choose the folder where you want to save the exported files.

---

### **Step 4: Convert `.gz.asset` Files to `.dat` Files**
1. Select the **"GZ.Asset to .DAT Converter"** tab.
2. Browse for the **"TextAsset"** folder exported by AssetStudio.
3. Select the folder where you want to save the converted `.dat` files.
4. Press **"Start Conversion"**.
5. The tool will convert all `.gz.asset` files into `.dat` files.

---

## **Requirements**
- **AssetStudio** (for inspecting and exporting assets)

---

## **Disclaimer**
This tool is for **educational purposes only**. It is not intended to infringe on any copyrights or violate any terms of service. Please use responsibly and respect the rights of all content creators.

---

## **License**
This project is open-source and licensed under the MIT License.

---

## **Credits**
- **[AssetStudio](https://github.com/Perfare/AssetStudio)** for asset inspection and exporting.
- Built with Python and `UnityPy`.
