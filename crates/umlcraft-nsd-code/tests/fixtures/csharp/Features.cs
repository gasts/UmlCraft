using System;
using System.Collections.Generic;
using System.IO;
using System.Threading.Tasks;

namespace Shop.Orders
{
    public class Features
    {
        private int count;
        private readonly object gate = new object();

        /// <summary>
        /// Adds two numbers.
        /// </summary>
        public int Add(int a, int b) => a + b;

        public void Log(string message) => Console.WriteLine(message);

        public Features(int start) : this()
        {
            count = start;
        }

        public Features()
        {
        }

        /// <summary>The number of items.</summary>
        public int Count
        {
            get { return count; }
            set { count = value; }
        }

        public int Auto { get; set; }

        public int Twice => count * 2;

        public int this[int i]
        {
            get => i * 2;
        }

        public static Features operator +(Features a, Features b)
        {
            return a;
        }

        ~Features()
        {
            count = 0;
        }

        public string Name(int day)
        {
            var name = day switch
            {
                1 => "Monday",
                2 or 3 => "Midweek",
                _ => "Other",
            };
            return name;
        }

        public string Size(int n) => n switch
        {
            < 0 => throw new ArgumentException("n"),
            0 => "zero",
            _ => "many",
        };

        public int Patterns(object o)
        {
            switch (o)
            {
                case int n when n > 0:
                    return n;
                case string s:
                    return s.Length;
                default:
                    return 0;
            }
        }

        public void Assignments(string s)
        {
            s ??= "none";
            var (a, b) = (1, 2);
            (a, b) = (b, a);
            count += a * b;
            count = s.Length > 3 ? a : b;
            foreach (var (k, v) in Pairs())
            {
                count++;
            }
        }

        public void Resources(string path)
        {
            using (var reader = Open(path))
            {
                reader.Read();
            }
            using var writer = Open(path);
            writer.Write();
            lock (gate)
            {
                count++;
            }
        }

        public IEnumerable<int> Numbers(int n)
        {
            for (int i = 0; i < n; i++)
            {
                if (i > 100)
                {
                    yield break;
                }
                yield return i;
            }
        }

        public void Jumps(int a)
        {
            switch (a)
            {
                case 1:
                    count = 1;
                    goto case 2;
                case 2:
                    count = 2;
                    break;
            }
        retry:
            if (count > 0)
            {
                count--;
                goto retry;
            }
        }

        public void Filters(Reader r)
        {
            try
            {
                r.Read();
            }
            catch (IOException e) when (e.Message != null)
            {
                Log(e.Message);
            }
            catch
            {
                throw;
            }
        }

        public void OneFilter(Reader r)
        {
            try
            {
                r.Read();
            }
            catch (IOException) when (count > 0)
            {
                this.Log("io");
            }
        }

        public async Task<int> Local(int n)
        {
            int Square(int x) => x * x;
            await Task.Delay(n);
            return Square(n);
        }

        #region Helpers
        private Reader Open(string path)
        {
#if DEBUG
            Log(path);
#else
            Trace(path);
#endif
            return null;
        }
        #endregion

        private IEnumerable<(int, int)> Pairs() => null;

        private void Trace(string path)
        {
            checked
            {
                count = count * 2;
            }
        }

        public struct Point
        {
            public int X;

            public Point(int x)
            {
                X = x;
            }
        }

        public record Pair(int A, int B)
        {
            public int Sum() => A + B;
        }

        public interface IShape
        {
            double Area()
            {
                return 0;
            }

            double Perimeter();
        }
    }
}
