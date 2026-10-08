// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.Intent;

import java.io.File;
import java.util.ArrayList;
import java.util.List;

import javax.xml.parsers.DocumentBuilderFactory;

import org.w3c.dom.Document;
import org.w3c.dom.Element;
import org.w3c.dom.Node;
import org.w3c.dom.NodeList;

/**
 * Attachments on a phone, on the JVM (android/jvm-checks/run.sh):
 * - Attachments.contentUri: the content:// address FileProvider gives a file
 *   of the attachments' folder, its name encoded as Uri.encode does, and
 *   none for a file elsewhere; Attachments.fileOf, FileProvider's way back;
 * - Attachments.type and view: what another app is handed (ACTION_VIEW, the
 *   address, the type, read only), never an installer;
 * - Attachments.saveAs: Android's question where to save;
 * - the provider's paths (res/xml/qtprovider_paths.xml) and the manifest:
 *   the root "attachments" is that folder of the cache alone, Qt's own roots
 *   stay, one FileProvider only, AttachmentSave not exported.
 * The repository's root comes as the property sioul.root.
 */
public final class AttachmentsCheck
{
    static int failed = 0;
    static int checked = 0;

    static void expect(String what, Object got, Object wanted)
    {
        checked++;
        if (!String.valueOf(got).equals(String.valueOf(wanted))) {
            failed++;
            System.out.println("FAIL " + what + ": " + got + " (expected " + wanted + ")");
        }
    }

    static List<Element> elements(Node parent, String name)
    {
        List<Element> found = new ArrayList<>();
        NodeList children = parent.getChildNodes();
        for (int i = 0; i < children.getLength(); i++) {
            if (children.item(i) instanceof Element && ((Element) children.item(i)).getTagName().equals(name))
                found.add((Element) children.item(i));
        }
        return found;
    }

    static Document read(File file) throws Exception
    {
        DocumentBuilderFactory factory = DocumentBuilderFactory.newInstance();
        factory.setNamespaceAware(false);
        return factory.newDocumentBuilder().parse(file);
    }

    public static void main(String[] args) throws Exception
    {
        final String pkg = "com.aurelienpierre.sioul";
        final String folder = "/data/data/com.aurelienpierre.sioul/cache/sioul/attachments";
        final String message = folder + "/0123456789abcdef/";
        final String start = "content://com.aurelienpierre.sioul.qtprovider/attachments/0123456789abcdef/";

        // The address, as FileProvider writes it: the root's name, then each part encoded.
        expect("plain", Attachments.contentUri(pkg, folder, message + "invoice.pdf"), start + "invoice.pdf");
        expect("spaces and accents", Attachments.contentUri(pkg, folder, message + "Facture n°12 été.pdf"), start + "Facture%20n%C2%B012%20%C3%A9t%C3%A9.pdf");
        expect("kept as they are", Attachments.contentUri(pkg, folder, message + "a_b-c!d.e~f'g(h)i*j"), start + "a_b-c!d.e~f'g(h)i*j");
        expect("what an address reads", Attachments.contentUri(pkg, folder, message + "a#b?c%d+e&f;g=h:i@j,k.pdf"), start + "a%23b%3Fc%25d%2Be%26f%3Bg%3Dh%3Ai%40j%2Ck.pdf");
        expect("an emoji", Attachments.contentUri(pkg, folder, message + "📄.pdf"), start + "%F0%9F%93%84.pdf");
        expect("folder with a slash", Attachments.contentUri(pkg, folder + "/", message + "invoice.pdf"), start + "invoice.pdf");
        // Nothing outside the folder: Sioul's settings, mail, another folder whose name begins alike, the folder itself.
        expect("settings refused", Attachments.contentUri(pkg, folder, "/data/data/com.aurelienpierre.sioul/files/config/sioul/config.toml"), null);
        expect("a folder alike refused", Attachments.contentUri(pkg, folder, folder + "-other/x.pdf"), null);
        expect("the folder itself refused", Attachments.contentUri(pkg, folder, folder), null);
        expect("the folder with a slash refused", Attachments.contentUri(pkg, folder, folder + "/"), null);
        expect("a way up refused", Attachments.contentUri(pkg, folder, message + "../../../files/config.toml"), null);
        expect("an empty part refused", Attachments.contentUri(pkg, folder, message + "/x.pdf"), null);
        expect("no package", Attachments.contentUri("", folder, message + "x.pdf"), null);

        // FileProvider's way back finds the same file, and none elsewhere.
        for (String name : new String[] {"invoice.pdf", "Facture n°12 été.pdf", "a#b?c%d+e&f.pdf", "100%25.txt", "📄 scan.jpeg"}) {
            String file = message + name;
            expect("back: " + name, Attachments.fileOf(pkg, folder, Attachments.contentUri(pkg, folder, file)), file);
        }
        expect("back: another root", Attachments.fileOf(pkg, folder, "content://com.aurelienpierre.sioul.qtprovider/files_path/config/sioul/config.toml"), null);
        expect("back: a way up", Attachments.fileOf(pkg, folder, start + "%2E%2E/%2E%2E/x"), null);
        expect("back: another authority", Attachments.fileOf(pkg, folder, "content://com.example.other/attachments/x"), null);
        expect("decode leaves a lone %", Attachments.decode("100%"), "100%");
        expect("decode reads UTF-8", Attachments.decode("%C3%A9t%C3%A9"), "été");

        // The type: Rust's by the extension, else Android's, else bytes; never Android's installer.
        expect("type: Rust's", Attachments.type("application/pdf", "application/x-other"), "application/pdf");
        expect("type: Android's for an unknown one", Attachments.type("application/octet-stream", "application/msword"), "application/msword");
        expect("type: none known", Attachments.type("", null), "application/octet-stream");
        expect("type: lower case", Attachments.type("Image/JPEG", null), "image/jpeg");
        expect("type: installer from Rust", Attachments.type("application/vnd.android.package-archive", null), null);
        expect("type: installer from Android", Attachments.type("application/octet-stream", "application/vnd.android.package-archive"), null);
        expect("type: installer in capitals", Attachments.type("Application/Vnd.Android.Package-Archive", null), null);

        // What another app is handed: ACTION_VIEW, the address, the type, read only, a task of its own.
        Attachments.View view = Attachments.view(pkg, folder, message + "Facture mars.pdf", "application/pdf");
        expect("view: action", view.action, Intent.ACTION_VIEW);
        expect("view: address", view.uri, start + "Facture%20mars.pdf");
        expect("view: type", view.type, "application/pdf");
        expect("view: read", (view.flags & Intent.FLAG_GRANT_READ_URI_PERMISSION) != 0, true);
        expect("view: never write", (view.flags & Intent.FLAG_GRANT_WRITE_URI_PERMISSION) != 0, false);
        expect("view: never kept", (view.flags & Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION) != 0, false);
        expect("view: never a prefix", (view.flags & Intent.FLAG_GRANT_PREFIX_URI_PERMISSION) != 0, false);
        expect("view: a task of its own", (view.flags & Intent.FLAG_ACTIVITY_NEW_TASK) != 0, true);
        expect("view: these flags alone", view.flags, Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_ACTIVITY_NEW_TASK);
        expect("view: a picture", Attachments.view(pkg, folder, message + "photo.jpg", Attachments.type("image/jpeg", null)).type, "image/jpeg");
        expect("view: an installer refused", Attachments.view(pkg, folder, message + "x.pdf", "application/vnd.android.package-archive"), null);
        expect("view: no type refused", Attachments.view(pkg, folder, message + "x.pdf", null), null);
        expect("view: outside refused", Attachments.view(pkg, folder, "/sdcard/Download/x.pdf", "application/pdf"), null);

        // Android's question where to save.
        Attachments.SaveAs saveAs = Attachments.saveAs("Facture mars.pdf", "application/pdf");
        expect("save: action", saveAs.action, Intent.ACTION_CREATE_DOCUMENT);
        expect("save: category", saveAs.category, Intent.CATEGORY_OPENABLE);
        expect("save: type", saveAs.type, "application/pdf");
        expect("save: name", saveAs.title, "Facture mars.pdf");
        expect("save: no type", Attachments.saveAs("x", "").type, "application/octet-stream");
        expect("save: no name", Attachments.saveAs("", "text/plain").title, "attachment");
        expect("save: an installer may be saved", Attachments.saveAs("app.apk", "application/vnd.android.package-archive").type, "application/vnd.android.package-archive");

        // The provider's paths: the root "attachments" is the attachments' folder of the cache alone; Qt's roots stay.
        String root = System.getProperty("sioul.root", "");
        File paths = new File(root, "android/package/res/xml/qtprovider_paths.xml");
        File manifest = new File(root, "android/package/AndroidManifest.xml");
        expect("paths: found", paths.isFile(), true);
        expect("manifest: found", manifest.isFile(), true);
        if (paths.isFile() && manifest.isFile()) {
            Element top = read(paths).getDocumentElement();
            expect("paths: top", top.getTagName(), "paths");
            List<Element> caches = elements(top, "cache-path");
            expect("paths: one cache root", caches.size(), 1);
            if (caches.size() == 1) {
                expect("paths: its name", caches.get(0).getAttribute("name"), Attachments.ROOT);
                expect("paths: its folder", caches.get(0).getAttribute("path").replaceAll("/$", ""), Attachments.FOLDER);
            }
            expect("paths: no root of the device", elements(top, "root-path").size(), 0);
            for (String[] qt : new String[][] {{"files-path", "files_path"}, {"external-path", "external_path"}, {"external-files-path", "external_files_path"}, {"external-cache-path", "external_cache_path"}}) {
                List<Element> kept = elements(top, qt[0]);
                expect("paths: Qt's " + qt[1] + " kept", kept.size() == 1 && kept.get(0).getAttribute("name").equals(qt[1]), true);
            }

            Element application = elements(read(manifest).getDocumentElement(), "application").get(0);
            int fileProviders = 0;
            for (Element provider : elements(application, "provider")) {
                if (!provider.getAttribute("android:name").equals("androidx.core.content.FileProvider"))
                    continue;
                fileProviders++;
                expect("provider: authority", provider.getAttribute("android:authorities"), "${applicationId}" + Attachments.AUTHORITY);
                expect("provider: not exported", provider.getAttribute("android:exported"), "false");
                expect("provider: grants", provider.getAttribute("android:grantUriPermissions"), "true");
                List<Element> meta = elements(provider, "meta-data");
                expect("provider: its paths", meta.size() == 1 && meta.get(0).getAttribute("android:resource").equals("@xml/qtprovider_paths"), true);
            }
            // Qt hands every other file to a FileProvider that is not its own when there are two (QAndroidPlatformServices).
            expect("provider: one FileProvider", fileProviders, 1);
            boolean declared = false;
            for (Element activity : elements(application, "activity")) {
                if (activity.getAttribute("android:name").equals("com.aurelienpierre.sioul.AttachmentSave")) {
                    declared = true;
                    expect("AttachmentSave: not exported", activity.getAttribute("android:exported"), "false");
                }
            }
            expect("AttachmentSave: declared", declared, true);
        }

        System.out.println("AttachmentsCheck: " + (checked - failed) + "/" + checked + " checks passed");
        if (failed > 0)
            System.exit(1);
    }
}
