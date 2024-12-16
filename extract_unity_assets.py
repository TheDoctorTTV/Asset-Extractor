import UnityPy
import os
import shutil
from datetime import datetime
import tkinter as tk
from tkinter import filedialog, messagebox

def extract_asset_bundle(file_path):
    # Use the script's directory for the output
    script_directory = os.path.dirname(os.path.abspath(__file__))
    output_directory = os.path.join(script_directory, "extracted_assets")
    raw_bundle_path = os.path.join(output_directory, "raw_asset_bundle.unity3d")
    
    # Ensure the output directory exists
    os.makedirs(output_directory, exist_ok=True)

    # Save the raw asset bundle for further inspection
    try:
        shutil.copyfile(file_path, raw_bundle_path)
        print(f"Raw asset bundle saved to: {raw_bundle_path}")
    except Exception as e:
        messagebox.showerror("Error", f"Failed to save the raw asset bundle: {str(e)}")
        return

    # Load the Unity Asset Bundle
    try:
        env = UnityPy.load(file_path)
        print("Unity Asset Bundle loaded successfully.")

        # Extract all objects and save them in the output directory
        object_count = 0
        for obj in env.objects:
            try:
                data = obj.read()
                # Generate object name safely
                object_name = getattr(data, "name", f"Unnamed_Object_{object_count}")
                output_path = os.path.join(output_directory, f"{object_name}_{data.type.name}_{object_count}")

                # Save objects if they have a save method
                if hasattr(data, "save"):
                    data.save(output_path)
                    print(f"Saved {data.type.name} to {output_path}")
                elif data.type.name == "TextAsset" and hasattr(data, "script"):
                    # Handle TextAsset objects
                    with open(f"{output_path}.txt", "w", encoding="utf-8") as f:
                        f.write(data.script)
                    print(f"Extracted TextAsset to {output_path}.txt")
            except Exception as e:
                print(f"Failed to process object {object_count}: {str(e)}")
            object_count += 1
    except Exception as e:
        print(f"Failed to extract assets: {str(e)}")
        messagebox.showerror("Error", "Failed to extract assets.")
        return

    print("Extraction complete. Files saved to:", output_directory)
    messagebox.showinfo("Success", f"Extraction complete! Files saved to:\n{output_directory}")

# GUI Setup
def browse_file():
    file_path = filedialog.askopenfilename(title="Select Unity Asset Bundle File")
    if file_path:
        extract_asset_bundle(file_path)

# Create GUI
root = tk.Tk()
root.title("Unity Asset Bundle Extractor")

# GUI Elements
label = tk.Label(root, text="Unity Asset Bundle Extractor", font=("Arial", 14))
label.pack(pady=10)

button_browse = tk.Button(root, text="Browse File", command=browse_file, font=("Arial", 12))
button_browse.pack(pady=5)

root.mainloop()
