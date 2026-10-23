using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using System.Windows.Forms;
using System.Xml.Linq;

namespace QuantSupport.Ribbon
{
    // Render the bundled Google Material Symbols vectors without a font or SVG dependency.
    internal static class MaterialIcons
    {
        private static readonly Dictionary<string, Bitmap> Images = new Dictionary<string, Bitmap>();
        private static readonly Color Ink = Color.FromArgb(23, 107, 117);
        private static readonly Regex Tokens = new Regex(@"[a-zA-Z]|[-+]?(?:\d*\.\d+|\d+\.?\d*)(?:[eE][-+]?\d+)?", RegexOptions.CultureInvariant);

        internal static Bitmap Get(string name, int size)
        {
            if (size < 12 || size > 96) throw new ArgumentOutOfRangeException(nameof(size));
            var key = name + ":" + size;
            lock (Images)
            {
                if (Images.TryGetValue(key, out var cached)) return cached;
                using (var stream = typeof(MaterialIcons).Assembly.GetManifestResourceStream("QuantSupport.Ribbon.Icons." + name + ".svg"))
                {
                    if (stream == null) throw new InvalidOperationException("Unknown Material icon: " + name);
                    var svg = XDocument.Load(stream).Root;
                    var box = svg.Attribute("viewBox").Value.Split(' ').Select(value => float.Parse(value, CultureInfo.InvariantCulture)).ToArray();
                    var image = new Bitmap(size, size, PixelFormat.Format32bppArgb);
                    using (var graphics = Graphics.FromImage(image))
                    using (var transform = new Matrix(size / box[2], 0, 0, size / box[3], -box[0] * size / box[2], -box[1] * size / box[3]))
                    using (var brush = new SolidBrush(Ink))
                    {
                        graphics.SmoothingMode = SmoothingMode.AntiAlias;
                        graphics.Transform = transform;
                        foreach (var element in svg.Elements().Where(value => value.Name.LocalName == "path"))
                            using (var path = ReadPath(element.Attribute("d").Value)) graphics.FillPath(brush, path);
                    }
                    Images.Add(key, image);
                    return image;
                }
            }
        }

        internal static object RibbonImage(string id)
        {
            var separator = id.LastIndexOf('_');
            if (separator < 0 || !int.TryParse(id.Substring(separator + 1), out var size))
                throw new InvalidOperationException("Invalid ribbon image: " + id);
            return Get(id.Substring(0, separator), size);
        }

        internal static void Apply(Button button, string name)
        {
            button.Image = Get(name, 16);
            button.ImageAlign = ContentAlignment.MiddleLeft;
            button.TextImageRelation = TextImageRelation.ImageBeforeText;
            button.Padding = new Padding(Math.Max(6, button.Padding.Left), button.Padding.Top, Math.Max(6, button.Padding.Right), button.Padding.Bottom);
        }

        internal static void Apply(TabControl tabs, params string[] names)
        {
            var images = new ImageList { ImageSize = new Size(16, 16), ColorDepth = ColorDepth.Depth32Bit };
            foreach (var name in names) images.Images.Add(name, Get(name, 16));
            tabs.ImageList = images;
            tabs.Disposed += (sender, args) => images.Dispose();
        }

        internal static void Release()
        {
            lock (Images)
            {
                foreach (var image in Images.Values) image.Dispose();
                Images.Clear();
            }
        }

        // The pinned symbols use move/line and quadratic Bezier commands only.
        // Unsupported SVG commands fail explicitly instead of drawing a partial glyph.
        private static GraphicsPath ReadPath(string data)
        {
            var tokens = Tokens.Matches(data).Cast<Match>().Select(match => match.Value).ToArray();
            var path = new GraphicsPath(FillMode.Winding);
            var position = new PointF();
            var start = new PointF();
            var control = new PointF();
            var index = 0;
            char command = ' ', previous = ' ';
            Func<float> number = () => float.Parse(tokens[index++], CultureInfo.InvariantCulture);
            Func<bool, PointF> point = relative => new PointF(number() + (relative ? position.X : 0), number() + (relative ? position.Y : 0));
            while (index < tokens.Length)
            {
                if (char.IsLetter(tokens[index][0])) command = tokens[index++][0];
                var relative = char.IsLower(command);
                var upper = char.ToUpperInvariant(command);
                PointF next;
                switch (upper)
                {
                    case 'M':
                        position = start = point(relative);
                        path.StartFigure();
                        command = relative ? 'l' : 'L';
                        break;
                    case 'L':
                    case 'H':
                    case 'V':
                        next = upper == 'L' ? point(relative) : upper == 'H'
                            ? new PointF(number() + (relative ? position.X : 0), position.Y)
                            : new PointF(position.X, number() + (relative ? position.Y : 0));
                        path.AddLine(position, next);
                        position = next;
                        break;
                    case 'Q':
                    case 'T':
                        control = upper == 'Q' ? point(relative) : (previous == 'Q' || previous == 'T'
                            ? new PointF(2 * position.X - control.X, 2 * position.Y - control.Y) : position);
                        next = point(relative);
                        path.AddBezier(position,
                            new PointF(position.X + (control.X - position.X) * 2 / 3, position.Y + (control.Y - position.Y) * 2 / 3),
                            new PointF(next.X + (control.X - next.X) * 2 / 3, next.Y + (control.Y - next.Y) * 2 / 3), next);
                        position = next;
                        break;
                    case 'Z':
                        path.CloseFigure();
                        position = start;
                        command = ' ';
                        break;
                    default:
                        path.Dispose();
                        throw new InvalidDataException("Unsupported Material icon path command: " + command);
                }
                previous = upper;
            }
            return path;
        }
    }
}
