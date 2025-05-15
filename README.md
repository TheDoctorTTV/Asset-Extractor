# **Beat Saber Mapping Extractor**

This tool allows you to extract and process Unity asset files (such as `.gz.asset`) for Beat Saber maps. It simplifies the process of converting and exporting assets for use or further modification.

> ⚠️ **Disclaimer**: This tool is provided for **educational purposes only**. Use it responsibly and respect the rights of content creators.

---

## **Features**
- Extract Unity asset bundles with a user-friendly GUI.
- Convert `.gz.asset` files to `.dat` files easily.
- Automates the process of preparing assets for use in tools like **AssetStudio**.

---

## **Tools Included**
1. **extract_unity_assets.exe**
   - Extracts raw Unity asset bundles and saves them for inspection.
   - Saves all extracted data into an organized folder.

2. **convert_gz_to_dat.exe**
   - Converts `.gz.asset` files into `.dat` files for further use.

---

## **How to Use the Tools**

### **Step 1: Extract Unity Assets**
1. Place your **asset bundle file** in the root folder (where the `.exe` files are located).
2. Run `extract_unity_assets.exe`.
3. A GUI will open. Press **"Browse File"**.
4. Locate and open your Unity asset file.
5. The tool will:
   - Extract the raw asset bundle into an `extracted_assets` folder.
   - Save the `raw_asset_bundle.unity3d` file for further use.

---

### **Step 2: Use AssetStudio to Export Assets**
1. Download **[AssetStudio](https://github.com/Perfare/AssetStudio)**.
2. Open `AssetStudioGUI.exe`.
3. Drag the `raw_asset_bundle.unity3d` file (from the `extracted_assets` folder) into AssetStudio.
4. In AssetStudio:
   - Click **"Export"** at the top.
   - Select **"All Assets"**.
   - Choose the folder where you want to save the exported files.

---

### **Step 3: Convert `.gz.asset` Files to `.dat` Files**
1. Run `convert_gz_to_dat.exe`.
2. When the GUI opens:
   - Browse for the **"TextAsset"** folder exported by AssetStudio.
   - Select the folder where you want to save the converted `.dat` files.
3. The tool will convert all `.gz.asset` files into `.dat` files.

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
